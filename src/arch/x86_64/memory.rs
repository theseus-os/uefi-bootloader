use crate::memory::{Frame, FrameAllocator, Page, PhysicalAddress, VirtualAddress};
use bit_field::BitField;
use goblin::elf64::program_header::ProgramHeader;
use uefi::table::boot::MemoryDescriptor;
use x86_64::structures::paging::{self, OffsetPageTable, PageTable, PageTableIndex};

pub use x86_64::structures::paging::PageTableFlags as PteFlags;

pub fn is_canonical_virtual_address(virt_addr: usize) -> bool {
    matches!(virt_addr.get_bits(47..64), 0 | 0b1_1111_1111_1111_1111)
}

pub const fn canonicalize_virtual_address(virt_addr: usize) -> usize {
    // match virt_addr.get_bit(47) {
    //     false => virt_addr.set_bits(48..64, 0),
    //     true =>  virt_addr.set_bits(48..64, 0xffff),
    // };

    // The below code is semantically equivalent to the above, but it works in const
    // functions.
    ((virt_addr << 16) as isize >> 16) as usize
}

pub fn is_canonical_physical_address(phys_addr: usize) -> bool {
    phys_addr.get_bits(52..64) == 0
}

pub const fn canonicalize_physical_address(phys_addr: usize) -> usize {
    phys_addr & 0x000F_FFFF_FFFF_FFFF
}

impl From<x86_64::VirtAddr> for VirtualAddress {
    fn from(value: x86_64::VirtAddr) -> Self {
        Self::new_canonical(value.as_u64() as usize)
    }
}

impl From<x86_64::PhysAddr> for PhysicalAddress {
    fn from(value: x86_64::PhysAddr) -> Self {
        Self::new_canonical(value.as_u64() as usize)
    }
}

impl From<Page> for paging::Page {
    fn from(page: Page) -> Self {
        Self::from_start_address(x86_64::VirtAddr::new(page.start_address().value() as u64))
            .unwrap()
    }
}

impl From<Frame> for paging::PhysFrame {
    fn from(frame: Frame) -> Self {
        Self::from_start_address(x86_64::PhysAddr::new(frame.start_address().value() as u64))
            .unwrap()
    }
}

pub struct PageAllocator {
    level_4_entries: [bool; 512],
}

impl PageAllocator {
    pub fn new() -> Self {
        let mut page_allocator = Self {
            level_4_entries: [false; 512],
        };
        // TODO: Why?
        page_allocator.level_4_entries[0] = true;

        page_allocator
    }

    fn get_free_entries(&mut self, num: u64) -> PageTableIndex {
        // Create an iterator over all available p4 indices with `num` contiguous free
        // entries.
        let mut free_entries = self
            .level_4_entries
            .windows(num as usize)
            .enumerate()
            .filter(|(_, entries)| entries.iter().all(|used| !used))
            .map(|(idx, _)| idx);

        let idx = free_entries
            .next()
            .expect("no usable level 4 entries found");

        // Mark the entries as used.
        for i in 0..num as usize {
            self.level_4_entries[idx + i] = true;
        }

        PageTableIndex::new(idx.try_into().unwrap())
    }

    pub fn get_free_address(&mut self, len: u64) -> VirtualAddress {
        const LEVEL_4_SIZE: u64 = 4096 * 512 * 512 * 512;
        let num_level_4_entries = (len + (LEVEL_4_SIZE - 1)) / LEVEL_4_SIZE;

        // TODO: Explain
        paging::Page::from_page_table_indices_1gib(
            self.get_free_entries(num_level_4_entries),
            PageTableIndex::new(0),
        )
        .start_address()
        .into()
    }

    pub fn mark_segment_as_used(&mut self, segment: ProgramHeader) {
        let start = VirtualAddress::new_canonical(segment.p_vaddr as usize);
        let end_inclusive = (start + segment.p_memsz as usize) - 1;

        let start_page = Page::containing_address(start);
        let end_page_inclusive = Page::containing_address(end_inclusive);

        for p4_index in start_page.p4_index()..=end_page_inclusive.p4_index() {
            self.level_4_entries[p4_index] = true;
        }
    }
}

unsafe impl<'a, I> paging::FrameAllocator<paging::page::Size4KiB> for FrameAllocator<'a, I>
where
    I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    fn allocate_frame(&mut self) -> Option<paging::PhysFrame<paging::page::Size4KiB>> {
        let frame = FrameAllocator::allocate_frame(self)?;
        Some(frame.into())
    }
}

pub struct Mapper {
    inner: OffsetPageTable<'static>,
}

impl Mapper {
    pub fn new<'a, I>(frame_allocator: &mut FrameAllocator<'a, I>) -> Self
    where
        I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
    {
        let frame = frame_allocator.allocate_frame().unwrap();
        // Physical memory is identity-mapped.
        let pointer = frame.start_address().value() as *mut _;
        unsafe { *pointer = PageTable::new() };
        let level_4_table = unsafe { &mut *pointer };
        Self {
            inner: unsafe { OffsetPageTable::new(level_4_table, x86_64::VirtAddr::zero()) },
        }
    }

    // TODO: This should take a shared reference to self.
    pub fn address(&mut self) -> PhysicalAddress {
        PhysicalAddress::new_canonical(self.inner.level_4_table() as *const _ as usize)
    }

    pub fn map<'a, I>(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PteFlags,
        frame_allocator: &mut FrameAllocator<'a, I>,
    ) where
        I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
    {
        // TODO: Unsafe
        unsafe {
            paging::Mapper::<paging::Size4KiB>::map_to(
                &mut self.inner,
                page.into(),
                frame.into(),
                flags,
                frame_allocator,
            )
        }
        .unwrap()
        .ignore();
    }
}
