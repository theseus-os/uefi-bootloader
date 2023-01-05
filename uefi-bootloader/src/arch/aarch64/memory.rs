use crate::memory::{Frame, FrameAllocator, Memory, Page, PhysicalAddress, VirtualAddress};
use bit_field::BitField;
use goblin::elf64::program_header::ProgramHeader;

/// On aarch64, VAs are composed of an ASID
/// which is 8 or 16 bits long depending
/// on MMU config. In Theseus, we use 8-bits
/// and the next 8 bits are unused.
/// Our ASID is zero, so a "canonical" VA has
/// the 16 most significant bits cleared.
pub fn is_canonical_virtual_address(virt_addr: usize) -> bool {
    virt_addr.get_bits(48..64) == 0
}

/// On aarch64, VAs are composed of an ASID
/// which is 8 or 16 bits long depending
/// on MMU config. In Theseus, we use 8-bits
/// and the next 8 bits are unused.
/// Our ASID is zero, so a "canonical" VA has
/// the 16 most significant bits cleared.
pub const fn canonicalize_virtual_address(virt_addr: usize) -> usize {
    virt_addr & 0x0000_FFFF_FFFF_FFFF
}

/// On aarch64, we configure the MMU to use 48-bit
/// physical addresses; "canonical" physical addresses
/// have the 16 most significant bits cleared.
pub fn is_canonical_physical_address(phys_addr: usize) -> bool {
    phys_addr.get_bits(48..64) == 0
}

/// On aarch64, we configure the MMU to use 48-bit
/// physical addresses; "canonical" physical addresses
/// have the 16 most significant bits cleared.
pub const fn canonicalize_physical_address(phys_addr: usize) -> usize {
    phys_addr & 0x0000_FFFF_FFFF_FFFF
}

pub fn set_up_arch_specific_mappings(_memory: &mut Memory) {
    unimplemented!();
}

bitflags::bitflags! {
    pub struct PteFlags: u64 {
        const PRESENT = 1;
        const WRITABLE = !(1 << 7);
        const NO_EXECUTE = (1 << 53) | (1 << 54);
    }
}

impl Page {
    const fn p0_index(&self) -> usize {
        (self.number >> 27) & 0x1ff
    }
}

pub struct PageAllocator {
    level_0_entries: [bool; 512],
}

impl PageAllocator {
    pub fn new() -> Self {
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

    pub fn get_free_address(&mut self, len: usize) -> VirtualAddress {
        const LEVEL_0_SIZE: usize = 4096 * 512 * 512 * 512;
        let num_level_0_entries = (len + (LEVEL_0_SIZE - 1)) / LEVEL_0_SIZE;

        let level_0_index = self.get_free_entries(num_level_0_entries as u64);
        let mut address = 0;

        address.set_bits(39..47, level_0_index);
        VirtualAddress::new(address).unwrap()
    }

    pub fn mark_segment_as_used(&mut self, segment: ProgramHeader) {
        let start = VirtualAddress::new_canonical(segment.p_vaddr as usize);
        let end_inclusive = (start + segment.p_memsz as usize) - 1;

        let start_page = Page::containing_address(start);
        let end_page_inclusive = Page::containing_address(end_inclusive);

        for p0_index in start_page.p0_index()..=end_page_inclusive.p0_index() {
            self.level_0_entries[p0_index] = true;
        }
    }
}

pub struct Mapper;

impl Mapper {
    pub fn new(_frame_allocator: &mut FrameAllocator) -> Self {
        Self
    }

    pub fn address(&mut self) -> PhysicalAddress {
        unimplemented!();
    }

    pub fn map(
        &mut self,
        _page: Page,
        _frame: Frame,
        _flags: PteFlags,
        _frame_allocator: &mut FrameAllocator,
    ) {
        unimplemented!()
    }
}
