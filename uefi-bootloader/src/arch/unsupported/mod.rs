use crate::KernelContext;

pub(crate) mod memory;

pub(crate) fn pre_context_switch_actions() {
    unimplemented!();
}

// The function needs to take ownership of the context so that it remains valid
// when we switch page tables.
#[allow(clippy::needless_pass_by_value)]
pub(crate) unsafe fn jump_to_kernel(_context: KernelContext) -> ! {
    unimplemented!();
}

pub(crate) fn halt() -> ! {
    unimplemented!();
}
