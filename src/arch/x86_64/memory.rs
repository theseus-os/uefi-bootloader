use bit_field::BitField;

pub fn is_canonical_virtual_address(virt_addr: usize) -> bool {
    matches!(virt_addr.get_bits(47..64), 0 | 0b1_1111_1111_1111_1111)
}

pub const fn canonicalize_virtual_address(virt_addr: usize) -> usize {
    // match virt_addr.get_bit(47) {
    //     false => virt_addr.set_bits(48..64, 0),
    //     true =>  virt_addr.set_bits(48..64, 0xffff),
    // };

    // The below code is semantically equivalent to the above, but it works in const
    // functions.
    ((virt_addr << 16) as isize >> 16) as usize
}

pub fn is_canonical_physical_address(phys_addr: usize) -> bool {
    phys_addr.get_bits(52..64) == 0
}

pub const fn canonicalize_physical_address(phys_addr: usize) -> usize {
    phys_addr & 0x000F_FFFF_FFFF_FFFF
}
