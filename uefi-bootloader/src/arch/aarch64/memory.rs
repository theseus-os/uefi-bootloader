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

pub struct PageAllocator;

impl PageAllocator {
    pub fn new() -> Self {
        Self
    }

    pub fn get_free_address(&mut self, _len: usize) -> VirtualAddress {
        unimplemented!();
    }

    pub fn mark_segment_as_used(&mut self, _segment: ProgramHeader) -> VirtualAddress {
        unimplemented!();
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
