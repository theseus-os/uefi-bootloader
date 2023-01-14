use crate::memory::{Frame, VirtualAddress};
use core::arch::asm;
use uefi_bootloader_api::BootInformation;

pub(crate) mod memory;

// The function needs to take ownership of the context so that it remains valid
// when we switch page tables.
#[allow(clippy::needless_pass_by_value)]
pub(crate) unsafe fn jump_to_kernel(
    page_table_frame: Frame,
    entry_point: VirtualAddress,
    boot_info: &'static BootInformation,
    stack_top: VirtualAddress,
) -> ! {
    // SAFETY: The caller guarantees that the context switch function is
    // identity-mapped, the stack pointer is mapped in the new page table, and the
    // kernel entry point is correct.
    unsafe {
        asm!(
            "mov cr3, {}; mov rsp, {}; jmp {}",
            in(reg) page_table_frame.start_address().value(),
            in(reg) stack_top.value(),
            in(reg) entry_point.value(),
            in("rdi") boot_info,
            options(noreturn),
        );
    }
}

pub(crate) fn halt() -> ! {
    loop {
        // SAFETY: These instructions will stop the CPU.
        unsafe { asm!("cli", "hlt") };
    }
}
