use crate::Context;
use core::arch::asm;
use cortex_a::{asm::barrier, registers::*};
use tock_registers::interfaces::{ReadWriteable, Writeable};

pub mod memory;

pub(crate) unsafe fn context_switch(context: Context) -> ! {
    set_as_active_page_table_root(context.page_table.start_address());
    configure_translation_registers();
    enable_mmu();
    unsafe {
        asm!(
            "br {}",
            in(reg) context.entry_point.value(),
            in("x0") context.boot_info,
            options(noreturn),
        );
    }
}

pub(crate) fn halt() -> ! {
    loop {
        unsafe { asm!("wfe") };
    }
}

const THESEUS_ASID: u16 = 0;

/// Enable the MMU using aarch64 registers
///
/// This uses the `SCTLR_EL1` register.
///
/// When the MMU is disabled, the CPU acts as
/// if a full-address-space identity mapping
/// was active. When it's enabled, the TTB0_EL1
/// register is expected to point to a valid
/// page table (using its physical address).
fn enable_mmu() {
    SCTLR_EL1.modify(SCTLR_EL1::M::Enable);
    barrier::isb(barrier::SY);
}

/// Configures paging for Theseus.
fn configure_translation_registers() {
    // The MAIR register holds up to 8 memory profiles;
    // each profile describes cacheability of the memory.
    // In Theseus, we currently use two profiles: one for
    // device memory (non-cacheable) and one for normal
    // memory (the usual RAM memory, cacheable).
    //
    // For more information on MAIR, See section D17.2.97
    // of [DDI0487l.a](https://l0.pm/arm-ddi0487l.a.pdf).
    MAIR_EL1.write(
        // Attribute 1 - Device.
        MAIR_EL1::Attr1_Device::nonGathering_nonReordering_EarlyWriteAck +

            // Attribute 0 - Cacheable normal DRAM.
            MAIR_EL1::Attr0_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc +
            MAIR_EL1::Attr0_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc,
    );

    // The translation control register contains most of
    // parameters for address translation; these parameters
    // notably define the format for page table entry flags.
    TCR_EL1.write(
        // TCR_EL1::DS is not exposed by `cortex_a`.
        // by default (cleared) it means we cannot
        // use 52-bit output-address mode.

        // Whether to use the top-byte of virtual
        // addresses for tagged addresses ("Ignored")
        // or to use them in the page table walk ("Used").
        // With our four-level paging, however, the top-byte
        // is not used for page table walks anyway.
        TCR_EL1::TBI0::Used
            // | TCR_EL1::TBI1::Used

            // Translation Granule Size = Page Size
            // => four kilobytes
            + TCR_EL1::TG0::KiB_4
            // + TCR_EL1::TG1::KiB_4

            // These fields could only be used if we had access
            // to the DS field, and if we set it to one. Indeed,
            // when DS=1, the shareability fields of page
            // descriptors are replaced by some bits of the output
            // address; the shareability is constant for the whole
            // page table: one for TTBR0 and one for TTBR1.
            // + TCR_EL1::SH0::Inner
            // + TCR_EL1::SH1::Inner

            // ASID Size. The upper 8 bits of TTBR0_EL1 and
            // TTBR1_EL1 are ignored by hardware for every
            // purpose except reading back the register, and
            // are treated as if they are all zeros for when
            // used for allocation and matching entries in the TLB.
            + TCR_EL1::AS::ASID8Bits

            // We currently output 48 bits of physical
            // address on aarch64.
            + TCR_EL1::IPS::Bits_48

            // Translation table walk disable. This bit controls
            // whether a translation table walk is performed on
            // a TLB miss.
            + TCR_EL1::EPD0::EnableTTBR0Walks
            // + TCR_EL1::EPD1::EnableTTBR1Walks

            // From which TTBR to read the ASID, when comparing the
            // current address space with the one from an address.
            + TCR_EL1::A1::TTBR0

            // Controls the size of the memory region addressed
            // by page table walks. We have to write 64 - (max
            // number of bits in a region address): 64 - 48 = 16.
            + TCR_EL1::T0SZ.val(16)
            // + TCR_EL1::T1SZ.val(16)

            // I (Nathan) don't understand when these flags are used at the moment
            // + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            // + TCR_EL1::ORGN1::NonCacheable
            // + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            // + TCR_EL1::IRGN1::NonCacheable

            // Allow the MMU to update the ACCESSED flag.
            + TCR_EL1::HA::Enable

            // Allow the MMU to update the DIRTY flag.
            + TCR_EL1::HD::Enable,
    );

    barrier::isb(barrier::SY);
}

/// Installs a page table in the TTBR CPU register
fn set_as_active_page_table_root(page_table: crate::memory::PhysicalAddress) {
    let page_table_addr = page_table.value() as u64;
    TTBR0_EL1.write(
        TTBR0_EL1::ASID.val(THESEUS_ASID as u64) + TTBR0_EL1::BADDR.val(page_table_addr >> 1),
    );

    barrier::isb(barrier::SY);
}
