// FIXME: This doesn't work.

use crate::KernelContext;
use core::arch::asm;
use cortex_a::{
    asm::barrier,
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Writeable};

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
    // disable the MMU
    SCTLR_EL1.modify(SCTLR_EL1::M::Disable);
    barrier::isb(barrier::SY);

    // install the new page table
    let page_table_addr = page_table as u64;
    TTBR0_EL1.write(
          TTBR0_EL1::ASID.val(ASID_ZERO as u64)
        + TTBR0_EL1::BADDR.val(page_table_addr >> 1)
    );

    configure_translation_registers();

    // re-enable the MMU
    barrier::isb(barrier::SY);
    SCTLR_EL1.modify(SCTLR_EL1::M::Enable);
    barrier::isb(barrier::SY);

    // flush the tlb
    asm!("tlbi aside1, {}", in(reg) 0usize);

    // flush the tlb
    asm!("mov sp, {}", in(reg) stack_top);

    // jump to the entry point defined by the kernel
    asm!(
        "br {}",
        in(reg) entry_point,
        in("x0") boot_info,
        options(noreturn)
    )
}

pub(crate) fn halt() -> ! {
    loop {
        // SAFETY: This instruction will stop the CPU.
        unsafe { asm!("wfe") };
    }
}

const ASID_ZERO: u16 = 0;

#[inline(always)]
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
}
