use bit_field::BitField;

/// On aarch64, VAs are composed of an ASID
/// which is 8 or 16 bits long depending
/// on MMU config. In Theseus, we use 8-bits
/// and the next 8 bits are unused.
/// Our ASID is zero, so a "canonical" VA has
/// the 16 most significant bits cleared.
pub fn is_canonical_virtual_address(virt_addr: usize) -> bool {
    match virt_addr.get_bits(48..64) {
        0 => true,
        _ => false,
    }
}

/// On aarch64, VAs are composed of an ASID
/// which is 8 or 16 bits long depending
/// on MMU config. In Theseus, we use 8-bits
/// and the next 8 bits are unused.
/// Our ASID is zero, so a "canonical" VA has
/// the 16 most significant bits cleared.
pub const fn canonicalize_virtual_address(virt_addr: usize) -> usize {
    virt_addr & 0x0000_FFFF_FFFF_FFFF
}

/// On aarch64, we configure the MMU to use 48-bit
/// physical addresses; "canonical" physical addresses
/// have the 16 most significant bits cleared.
pub fn is_canonical_physical_address(phys_addr: usize) -> bool {
    match phys_addr.get_bits(48..64) {
        0 => true,
        _ => false,
    }
}

/// On aarch64, we configure the MMU to use 48-bit
/// physical addresses; "canonical" physical addresses
/// have the 16 most significant bits cleared.
pub const fn canonicalize_physical_address(phys_addr: usize) -> usize {
    phys_addr & 0x0000_FFFF_FFFF_FFFF
}
