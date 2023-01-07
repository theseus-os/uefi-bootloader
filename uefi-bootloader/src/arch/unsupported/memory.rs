use crate::{
    memory::{Frame, FrameAllocator, Page, VirtualAddress},
    RuntimeContext,
};
use goblin::elf64::program_header::ProgramHeader;

pub(crate) fn is_canonical_virtual_address(_virtual_address: usize) -> bool {
    unimplemented!();
}

pub(crate) const fn canonicalize_virtual_address(_virtual_address: usize) -> usize {
    unimplemented!();
}

pub(crate) fn is_canonical_physical_address(_physical_address: usize) -> bool {
    unimplemented!();
}

pub(crate) const fn canonicalize_physical_address(_physical_address: usize) -> usize {
    unimplemented!();
}

pub(crate) fn set_up_arch_specific_mappings(_context: &mut RuntimeContext) {
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
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn get_free_address(&mut self, _len: usize) -> VirtualAddress {
        unimplemented!();
    }

    pub(crate) fn mark_segment_as_used(&mut self, _segment: ProgramHeader) {
        unimplemented!();
    }
}

pub struct Mapper;

impl Mapper {
    pub(crate) fn new<T>(_frame_allocator: &mut T) -> Self
    where
        T: FrameAllocator,
    {
        unimplemented!();
    }

    pub(crate) fn current<T>(_frame_allocator: &mut T) -> Self
    where
        T: FrameAllocator,
    {
        unimplemented!();
    }

    pub(crate) fn frame(&mut self) -> Frame {
        unimplemented!();
    }

    pub(crate) fn map<T>(
        &mut self,
        _page: Page,
        _frame: Frame,
        _flags: PteFlags,
        _frame_allocator: &mut T,
    ) where
        T: FrameAllocator,
    {
        unimplemented!()
    }
}
