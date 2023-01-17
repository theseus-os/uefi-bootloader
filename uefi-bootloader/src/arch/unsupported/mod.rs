use crate::memory::{Frame, VirtualAddress};
use uefi_bootloader_api::BootInformation;

pub(crate) mod memory;

// The function needs to take ownership of the context so that it remains valid
// when we switch page tables.
#[allow(clippy::needless_pass_by_value)]
pub(crate) unsafe fn jump_to_kernel(
    _page_table_frame: Frame,
    _entry_point: VirtualAddress,
    _boot_info: &'static BootInformation,
    _stack_top: VirtualAddress,
) -> ! {
    unimplemented!();
}

pub(crate) fn halt() -> ! {
    unimplemented!();
}
