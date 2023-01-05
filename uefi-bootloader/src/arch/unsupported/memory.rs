use crate::memory::{Frame, FrameAllocator, Memory, Page, PhysicalAddress, VirtualAddress};
use goblin::elf64::program_header::ProgramHeader;

pub fn is_canonical_virtual_address(_virtual_address: usize) -> bool {
    unimplemented!();
}

pub const fn canonicalize_virtual_address(_virtual_address: usize) -> usize {
    unimplemented!();
}

pub fn is_canonical_physical_address(_physical_address: usize) -> bool {
    unimplemented!();
}

pub const fn canonicalize_physical_address(_physical_address: usize) -> usize {
    unimplemented!();
}

pub fn set_up_arch_specific_mappings(_memory: &mut Memory) {
    unimplemented!();
}

bitflags::bitflags! {
    pub struct PteFlags: u64 {
        const PRESENT = 1;
        const WRITABLE = 2;
        const NO_EXECUTE = 3;
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

    pub fn mark_segment_as_used(&mut self, _segment: ProgramHeader) {
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
