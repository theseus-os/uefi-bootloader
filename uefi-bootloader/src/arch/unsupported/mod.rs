use crate::KernelContext;

pub mod memory;

pub(crate) fn pre_context_switch_actions() {
    unimplemented!();
}

pub(crate) unsafe fn context_switch(_context: KernelContext) -> ! {
    unimplemented!();
}

pub(crate) fn halt() -> ! {
    unimplemented!();
}
