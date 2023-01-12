// FIXME: This doesn't work.

use crate::KernelContext;
use core::arch::asm;
use cortex_a::{
    asm::barrier,
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Writeable};

pub(crate) mod memory;

pub(crate) fn pre_context_switch_actions() {
    enable_mmu();
    configure_translation_registers();
}

// The function needs to take ownership of the context so that it remains valid
// when we switch page tables.
#[allow(clippy::needless_pass_by_value)]
pub(crate) unsafe fn jump_to_kernel(context: KernelContext) -> ! {
    // SAFETY: The caller guarantees that the context switch function is
    // identity-mapped, the stack pointer is mapped in the new page table, and the
    // kernel entry point is correct.
    unsafe {
        // TODO: Set stack pointer, and jump to entry point.
        core::arch::asm!(
            "msr ttbr0_el1, {}",
            "tlbi alle1",
            "dsb ish",
            "isb",
            "2:",
            "mov x2, 0xdead",
            "b 2b",
            in(reg) (context.page_table_frame.start_address().value() as u64),
            options(noreturn),
        )
    };
}

pub(crate) fn halt() -> ! {
    loop {
        // SAFETY: This instruction will stop the CPU.
        unsafe { asm!("wfe") };
    }
}

const THESEUS_ASID: u16 = 0;

fn enable_mmu() {
    SCTLR_EL1.modify(SCTLR_EL1::M::Enable);
    barrier::isb(barrier::SY);
}

fn configure_translation_registers() {
    MAIR_EL1.write(
        MAIR_EL1::Attr1_Device::nonGathering_nonReordering_EarlyWriteAck
            + MAIR_EL1::Attr0_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
            + MAIR_EL1::Attr0_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc,
    );

    TCR_EL1.write(
        TCR_EL1::TBI0::Used
            + TCR_EL1::TG0::KiB_4
            + TCR_EL1::AS::ASID8Bits
            + TCR_EL1::IPS::Bits_48
            + TCR_EL1::EPD0::EnableTTBR0Walks
            + TCR_EL1::A1::TTBR0
            + TCR_EL1::T0SZ.val(16)
            + TCR_EL1::HA::Enable
            + TCR_EL1::HD::Enable,
    );

    barrier::isb(barrier::SY);
}
