use crate::{
    memory::{Frame, FrameAllocator, Page, PhysicalAddress, VirtualAddress, PAGE_SIZE},
    RuntimeContext,
};
use bit_field::BitField;
use core::{
    ops::{Index, IndexMut},
    ptr,
};
use cortex_a::{asm::barrier, registers::TTBR0_EL1};
use goblin::elf64::program_header::ProgramHeader;

/// On aarch64, VAs are composed of an ASID
/// which is 8 or 16 bits long depending
/// on MMU config. In Theseus, we use 8-bits
/// and the next 8 bits are unused.
/// Our ASID is zero, so a "canonical" VA has
/// the 16 most significant bits cleared.
pub(crate) fn is_canonical_virtual_address(virt_addr: usize) -> bool {
    virt_addr.get_bits(48..64) == 0
}

/// On aarch64, VAs are composed of an ASID
/// which is 8 or 16 bits long depending
/// on MMU config. In Theseus, we use 8-bits
/// and the next 8 bits are unused.
/// Our ASID is zero, so a "canonical" VA has
/// the 16 most significant bits cleared.
pub(crate) const fn canonicalize_virtual_address(virt_addr: usize) -> usize {
    virt_addr & 0x0000_FFFF_FFFF_FFFF
}

/// On aarch64, we configure the MMU to use 48-bit
/// physical addresses; "canonical" physical addresses
/// have the 16 most significant bits cleared.
pub(crate) fn is_canonical_physical_address(phys_addr: usize) -> bool {
    phys_addr.get_bits(48..64) == 0
}

/// On aarch64, we configure the MMU to use 48-bit
/// physical addresses; "canonical" physical addresses
/// have the 16 most significant bits cleared.
pub(crate) const fn canonicalize_physical_address(phys_addr: usize) -> usize {
    phys_addr & 0x0000_FFFF_FFFF_FFFF
}

pub(crate) fn set_up_arch_specific_mappings(_: &mut RuntimeContext) {}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PteFlags(u64);

impl PteFlags {
    pub(crate) fn new() -> Self {
        Self(0)
    }

    pub(crate) fn present(self, enable: bool) -> Self {
        const BITS: u64 = 1;

        if enable {
            Self(self.0 | BITS)
        } else {
            Self(self.0 & !(BITS))
        }
    }

    fn page_descriptor(self, enable: bool) -> Self {
        const BITS: u64 = 1 << 1;

        if enable {
            Self(self.0 | BITS)
        } else {
            Self(self.0 & !(BITS))
        }
    }

    pub(crate) fn writable(self, enable: bool) -> Self {
        const BITS: u64 = 1 << 7;

        if enable {
            Self(self.0 & !(BITS))
        } else {
            Self(self.0 | BITS)
        }
    }

    pub(crate) fn no_execute(self, enable: bool) -> Self {
        const BITS: u64 = (1 << 53) | (1 << 54);

        if enable {
            Self(self.0 | BITS)
        } else {
            Self(self.0 & !(BITS))
        }
    }
}

impl Page {
    const fn p0_index(self) -> usize {
        (self.number >> 27) & 0x1ff
    }

    const fn p1_index(self) -> usize {
        (self.number >> 18) & 0x1ff
    }

    const fn p2_index(self) -> usize {
        (self.number >> 9) & 0x1ff
    }

    const fn p3_index(self) -> usize {
        self.number & 0x1ff
    }
}

pub(crate) struct PageAllocator {
    level_0_entries: [bool; 512],
}

impl PageAllocator {
    pub(crate) fn new() -> Self {
        let mut page_allocator = Self {
            level_0_entries: [false; 512],
        };
        page_allocator.level_0_entries[0] = true;

        page_allocator
    }

    fn get_free_entries(&mut self, num: u64) -> usize {
        // Create an iterator over all available p4 indices with `num` contiguous free
        // entries.
        let mut free_entries = self
            .level_0_entries
            .windows(num as usize)
            .enumerate()
            .filter(|(_, entries)| entries.iter().all(|used| !used))
            .map(|(idx, _)| idx);

        let idx = free_entries
            .next()
            .expect("no usable level 0 entries found");

        // Mark the entries as used.
        for i in 0..num as usize {
            self.level_0_entries[idx + i] = true;
        }

        idx
    }

    pub(crate) fn get_free_address(&mut self, len: usize) -> VirtualAddress {
        const LEVEL_0_SIZE: usize = 4096 * 512 * 512 * 512;
        let num_level_0_entries = (len + (LEVEL_0_SIZE - 1)) / LEVEL_0_SIZE;

        let level_0_index = self.get_free_entries(num_level_0_entries as u64);
        let mut address = 0;

        address.set_bits(39..47, level_0_index);
        VirtualAddress::new(address).expect("allocated invalid virtual address")
    }

    pub(crate) fn mark_segment_as_used(&mut self, segment: &ProgramHeader) {
        let start = VirtualAddress::new_canonical(segment.p_vaddr as usize);
        let end_inclusive = (start + segment.p_memsz as usize) - 1;

        let start_page = Page::containing_address(start);
        let end_page_inclusive = Page::containing_address(end_inclusive);

        for p0_index in start_page.p0_index()..=end_page_inclusive.p0_index() {
            self.level_0_entries[p0_index] = true;
        }
    }
}

pub(crate) struct Mapper {
    level_zero_page_table: &'static mut PageTable,
}

impl Mapper {
    pub(crate) fn new<T>(frame_allocator: &mut T) -> Self
    where
        T: FrameAllocator,
    {
        let address = frame_allocator
            .allocate_frame()
            .expect("failed to allocate frame for page table")
            .start_address()
            .value() as *mut PageTable;
        unsafe { ptr::write_bytes(address, 0, 1) };
        Self {
            level_zero_page_table: unsafe { &mut *address },
        }
    }

    pub(crate) fn current<T>(_frame_allocator: &mut T) -> Self
    where
        T: FrameAllocator,
    {
        let address = PhysicalAddress::new_canonical(TTBR0_EL1.get_baddr() as usize).value()
            as *mut PageTable;
        Self {
            level_zero_page_table: unsafe { &mut *address },
        }
    }

    pub(crate) fn frame(&mut self) -> Frame {
        Frame::containing_address(PhysicalAddress::new_canonical(
            self.level_zero_page_table as *const _ as usize,
        ))
    }

    pub(crate) fn map<T>(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PteFlags,
        frame_allocator: &mut T,
    ) where
        T: FrameAllocator,
    {
        let page_table_flags = PteFlags::new()
            .present(true)
            .page_descriptor(true)
            .writable(true)
            .no_execute(true);

        let level_1 = unsafe {
            self.level_zero_page_table.create_next_table(
                page.p0_index(),
                page_table_flags,
                frame_allocator,
            )
        };
        let level_2 = unsafe {
            level_1.create_next_table(page.p1_index(), page_table_flags, frame_allocator)
        };
        let level_3 = unsafe {
            level_2.create_next_table(page.p2_index(), page_table_flags, frame_allocator)
        };

        level_3[page.p3_index()].set(frame, flags.page_descriptor(true));

        barrier::isb(barrier::SY);
    }
}

#[derive(Debug)]
#[repr(C, align(4096))]
struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    unsafe fn create_next_table<T>(
        &mut self,
        index: usize,
        page_table_flags: PteFlags,
        frame_allocator: &mut T,
    ) -> &mut PageTable
    where
        T: FrameAllocator,
    {
        let entry = &mut self[index];
        if entry.is_unused() {
            let frame = frame_allocator
                .allocate_frame()
                .expect("failed to allocate frame for page table");
            unsafe { ptr::write_bytes(frame.start_address().value() as *mut PageTable, 0, 1) };
            entry.set(frame, page_table_flags);
        }
        unsafe { entry.as_page_table() }
    }
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
struct PageTableEntry(u64);

impl PageTableEntry {
    fn is_unused(&self) -> bool {
        self.0 == 0
    }

    fn output_address(&self) -> PhysicalAddress {
        PhysicalAddress::new_canonical(self.0 as usize & (!(PAGE_SIZE - 1) & !(0xffff << 48)))
    }

    fn set(&mut self, frame: Frame, flags: PteFlags) {
        // self.0 = frame.start_address().value() as u64 | flags.0;
        self.0 = frame.start_address().value() as u64 | 0x70f;
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn as_page_table(&self) -> &'static mut PageTable {
        // SAFETY: Address validity guaranteed by caller.
        unsafe { &mut *((self.0.get_bits(12..52) << 12) as *mut _) }
    }
}
