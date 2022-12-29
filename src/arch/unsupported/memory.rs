use crate::memory::{Frame, Page};

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

pub struct Mapper;

impl Mapper {
    pub fn new() -> Self {
        Self
    }

    unsafe fn map_to(&mut self, _page: Page, _frame: Frame) {
        unimplemented!()
    }
}
