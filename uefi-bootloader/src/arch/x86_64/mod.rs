use crate::KernelContext;
use core::arch::asm;

pub(crate) mod memory;

// The function needs to take ownership of the context so that it remains valid
// when we switch page tables.
#[allow(clippy::needless_pass_by_value)]
pub(crate) unsafe fn jump_to_kernel(
    page_table_frame: *const (),
    entry_point: *const (),
    boot_info: *const (),
    stack_top: *const (),
) -> ! {
    // SAFETY: The caller guarantees that the context switch function is
    // identity-mapped, the stack pointer is mapped in the new page table, and the
    // kernel entry point is correct.
    unsafe {
        asm!(
            "mov cr3, {}; mov rsp, {}; jmp {}",
            in(reg) page_table_frame,
            in(reg) stack_top,
            in(reg) entry_point,
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
