use crate::Context;

pub mod memory;

pub(crate) unsafe fn context_switch(_context: Context) -> ! {
    unimplemented!();
}

pub(crate) fn halt() -> ! {
    unimplemented!();
}
