// TODO: Depend on memory_structs

use crate::arch::memory as imp;
use core::{
    cmp::{max, min},
    fmt,
    iter::Step,
    ops::{Add, AddAssign, Deref, DerefMut, RangeInclusive, Sub, SubAssign},
};
use derive_more::{
    Add, AddAssign, Binary, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign,
    LowerHex, Octal, Sub, SubAssign, UpperHex,
};
use goblin::elf64::program_header::ProgramHeader;
use paste::paste;
use uefi::table::boot::{MemoryDescriptor, MemoryType};
use zerocopy::FromBytes;

pub use imp::PteFlags;

const PAGE_SIZE: usize = 4096;
const MAX_PAGE_NUMBER: usize = usize::MAX / PAGE_SIZE;

/// A macro for defining `VirtualAddress` and `PhysicalAddress` structs
/// and implementing their common traits, which are generally identical.
macro_rules! implement_address {
    ($TypeName:ident, $desc:literal, $prefix:literal, $is_canonical:path, $canonicalize:path, $chunk:ident) => {
        paste! { // using the paste crate's macro for easy concatenation

            #[doc = "A " $desc " memory address, which is a `usize` under the hood."]
            #[derive(
                Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
                Binary, Octal, LowerHex, UpperHex,
                BitAnd, BitOr, BitXor, BitAndAssign, BitOrAssign, BitXorAssign,
                Add, Sub, AddAssign, SubAssign,
                FromBytes,
            )]
            #[repr(transparent)]
            pub struct $TypeName(usize);

            impl $TypeName {
                #[doc = "Creates a new `" $TypeName "`, returning an error if the address is not canonical.\n\n \
                    This is useful for checking whether an address is valid before using it. 
                    For example, on x86_64, virtual addresses are canonical
                    if their upper bits `(64:48]` are sign-extended from bit 47,
                    and physical addresses are canonical if their upper bits `(64:52]` are 0."]
                pub fn new(addr: usize) -> Option<$TypeName> {
                    if $is_canonical(addr) { Some($TypeName(addr)) } else { None }
                }

                #[doc = "Creates a new `" $TypeName "` that is guaranteed to be canonical."]
                pub const fn new_canonical(addr: usize) -> $TypeName {
                    $TypeName($canonicalize(addr))
                }

                #[doc = "Creates a new `" $TypeName "` with a value 0."]
                pub const fn zero() -> $TypeName {
                    $TypeName(0)
                }

                #[doc = "Returns the underlying `usize` value for this `" $TypeName "`."]
                #[inline]
                pub const fn value(&self) -> usize {
                    self.0
                }

                #[doc = "Returns the offset from the " $chunk " boundary specified by this `"
                    $TypeName ".\n\n \
                    For example, if the [`PAGE_SIZE`] is 4096 (4KiB), then this will return
                    the least significant 12 bits `(12:0]` of this `" $TypeName "`."]
                pub const fn [<$chunk _offset>](&self) -> usize {
                    self.0 & (PAGE_SIZE - 1)
                }
            }
            impl fmt::Debug for $TypeName {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, concat!($prefix, "{:#X}"), self.0)
                }
            }
            impl fmt::Display for $TypeName {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
            impl fmt::Pointer for $TypeName {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
            impl Add<usize> for $TypeName {
                type Output = $TypeName;
                fn add(self, rhs: usize) -> $TypeName {
                    $TypeName::new_canonical(self.0.saturating_add(rhs))
                }
            }
            impl AddAssign<usize> for $TypeName {
                fn add_assign(&mut self, rhs: usize) {
                    *self = $TypeName::new_canonical(self.0.saturating_add(rhs));
                }
            }
            impl Sub<usize> for $TypeName {
                type Output = $TypeName;
                fn sub(self, rhs: usize) -> $TypeName {
                    $TypeName::new_canonical(self.0.saturating_sub(rhs))
                }
            }
            impl SubAssign<usize> for $TypeName {
                fn sub_assign(&mut self, rhs: usize) {
                    *self = $TypeName::new_canonical(self.0.saturating_sub(rhs));
                }
            }
            impl From<$TypeName> for usize {
                #[inline]
                fn from(value: $TypeName) -> Self {
                    value.0
                }
            }
        }
    };
}

implement_address!(
    VirtualAddress,
    "virtual",
    "v",
    imp::is_canonical_virtual_address,
    imp::canonicalize_virtual_address,
    page
);

implement_address!(
    PhysicalAddress,
    "physical",
    "p",
    imp::is_canonical_physical_address,
    imp::canonicalize_physical_address,
    frame
);

/// A macro for defining `Page` and `Frame` structs
/// and implementing their common traits, which are generally identical.
macro_rules! implement_page_frame {
    ($TypeName:ident, $desc:literal, $prefix:literal, $address:ident) => {
        paste! { // using the paste crate's macro for easy concatenation

            #[doc = "A `" $TypeName "` is a chunk of **" $desc "** memory aligned to a [`PAGE_SIZE`] boundary."]
            #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
            pub struct $TypeName {
                number: usize,
            }

            impl $TypeName {
                #[doc = "Returns the `" $address "` at the start of this `" $TypeName "`."]
                pub const fn start_address(&self) -> $address {
                    $address::new_canonical(self.number * PAGE_SIZE)
                }

                #[doc = "Returns the number of this `" $TypeName "`."]
                #[inline(always)]
                pub const fn number(&self) -> usize {
                    self.number
                }

                #[doc = "Returns the `" $TypeName "` containing the given `" $address "`."]
                pub const fn containing_address(addr: $address) -> $TypeName {
                    $TypeName {
                        number: addr.value() / PAGE_SIZE,
                    }
                }
            }
            impl fmt::Debug for $TypeName {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, concat!(stringify!($TypeName), "(", $prefix, "{:#X})"), self.start_address())
                }
            }
            impl Add<usize> for $TypeName {
                type Output = $TypeName;
                fn add(self, rhs: usize) -> $TypeName {
                    // cannot exceed max page number (which is also max frame number)
                    $TypeName {
                        number: core::cmp::min(MAX_PAGE_NUMBER, self.number.saturating_add(rhs)),
                    }
                }
            }
            impl AddAssign<usize> for $TypeName {
                fn add_assign(&mut self, rhs: usize) {
                    *self = $TypeName {
                        number: core::cmp::min(MAX_PAGE_NUMBER, self.number.saturating_add(rhs)),
                    };
                }
            }
            impl Sub<usize> for $TypeName {
                type Output = $TypeName;
                fn sub(self, rhs: usize) -> $TypeName {
                    $TypeName {
                        number: self.number.saturating_sub(rhs),
                    }
                }
            }
            impl SubAssign<usize> for $TypeName {
                fn sub_assign(&mut self, rhs: usize) {
                    *self = $TypeName {
                        number: self.number.saturating_sub(rhs),
                    };
                }
            }
            #[doc = "Implementing `Step` allows `" $TypeName "` to be used in an [`Iterator`]."]
            impl Step for $TypeName {
                #[inline]
                fn steps_between(start: &$TypeName, end: &$TypeName) -> Option<usize> {
                    Step::steps_between(&start.number, &end.number)
                }
                #[inline]
                fn forward_checked(start: $TypeName, count: usize) -> Option<$TypeName> {
                    Step::forward_checked(start.number, count).map(|n| $TypeName { number: n })
                }
                #[inline]
                fn backward_checked(start: $TypeName, count: usize) -> Option<$TypeName> {
                    Step::backward_checked(start.number, count).map(|n| $TypeName { number: n })
                }
            }
        }
    };
}

implement_page_frame!(Page, "virtual", "v", VirtualAddress);
implement_page_frame!(Frame, "physical", "p", PhysicalAddress);

// Implement other functions for the `Page` type that aren't relevant for
// `Frame.
impl Page {
    /// Returns the 9-bit part of this `Page`'s [`VirtualAddress`] that is the
    /// index into the P4 page table entries list.
    pub const fn p4_index(&self) -> usize {
        (self.number >> 27) & 0x1FF
    }

    /// Returns the 9-bit part of this `Page`'s [`VirtualAddress`] that is the
    /// index into the P3 page table entries list.
    pub const fn p3_index(&self) -> usize {
        (self.number >> 18) & 0x1FF
    }

    /// Returns the 9-bit part of this `Page`'s [`VirtualAddress`] that is the
    /// index into the P2 page table entries list.
    pub const fn p2_index(&self) -> usize {
        (self.number >> 9) & 0x1FF
    }

    /// Returns the 9-bit part of this `Page`'s [`VirtualAddress`] that is the
    /// index into the P1 page table entries list.
    ///
    /// Using this returned `usize` value as an index into the P1 entries list
    /// will give you the final PTE, from which you can extract the mapped
    /// [`Frame`]  using `PageTableEntry::pointed_frame()`.
    pub const fn p1_index(&self) -> usize {
        self.number & 0x1FF
    }
}

/// A macro for defining `PageRange` and `FrameRange` structs
/// and implementing their common traits, which are generally identical.
macro_rules! implement_page_frame_range {
    ($TypeName:ident, $desc:literal, $short:ident, $chunk:ident, $address:ident) => {
        paste! { // using the paste crate's macro for easy concatenation

            #[doc = "A range of [`" $chunk "`]s that are contiguous in " $desc " memory."]
            #[derive(Clone, PartialEq, Eq)]
            pub struct $TypeName(RangeInclusive<$chunk>);

            impl $TypeName {
                #[doc = "Creates a new range of [`" $chunk "`]s that spans from `start` to `end`, both inclusive bounds."]
                pub const fn new(start: $chunk, end: $chunk) -> $TypeName {
                    $TypeName(RangeInclusive::new(start, end))
                }

                #[doc = "Creates a `" $TypeName "` that will always yield `None` when iterated."]
                pub const fn empty() -> $TypeName {
                    $TypeName::new($chunk { number: 1 }, $chunk { number: 0 })
                }

                #[doc = "A convenience method for creating a new `" $TypeName "` that spans \
                    all [`" $chunk "`]s from the given [`" $address "`] to an end bound based on the given size."]
                pub fn [<from_ $short _addr>](starting_addr: $address, size_in_bytes: usize) -> $TypeName {
                    assert!(size_in_bytes > 0);
                    let start = $chunk::containing_address(starting_addr);
                    // The end bound is inclusive, hence the -1. Parentheses are needed to avoid overflow.
                    let end = $chunk::containing_address(starting_addr + (size_in_bytes - 1));
                    $TypeName::new(start, end)
                }

                #[doc = "Returns the [`" $address "`] of the starting [`" $chunk "`] in this `" $TypeName "`."]
                pub const fn start_address(&self) -> $address {
                    self.0.start().start_address()
                }

                #[doc = "Returns the number of [`" $chunk "`]s covered by this iterator.\n\n \
                    Use this instead of [`Iterator::count()`] method. \
                    This is instant, because it doesn't need to iterate over each entry, unlike normal iterators."]
                pub const fn [<size_in_ $chunk:lower s>](&self) -> usize {
                    // add 1 because it's an inclusive range
                    (self.0.end().number + 1).saturating_sub(self.0.start().number)
                }

                /// Returns the size of this range in number of bytes.
                pub const fn size_in_bytes(&self) -> usize {
                    self.[<size_in_ $chunk:lower s>]() * PAGE_SIZE
                }

                #[doc = "Returns `true` if this `" $TypeName "` contains the given [`" $address "`]."]
                pub fn contains_address(&self, addr: $address) -> bool {
                    self.0.contains(&$chunk::containing_address(addr))
                }

                #[doc = "Returns the offset of the given [`" $address "`] within this `" $TypeName "`, \
                    i.e., `addr - self.start_address()`.\n\n \
                    If the given `addr` is not covered by this range of [`" $chunk "`]s, this returns `None`.\n\n \
                    # Examples\n \
                    If the range covers addresses `0x2000` to `0x4000`, then `offset_of_address(0x3500)` would return `Some(0x1500)`."]
                pub fn offset_of_address(&self, addr: $address) -> Option<usize> {
                    if self.contains_address(addr) {
                        Some(addr.value() - self.start_address().value())
                    } else {
                        None
                    }
                }

                #[doc = "Returns the [`" $address "`] at the given `offset` into this `" $TypeName "`within this `" $TypeName "`, \
                    i.e., `addr - self.start_address()`.\n\n \
                    If the given `offset` is not within this range of [`" $chunk "`]s, this returns `None`.\n\n \
                    # Examples\n \
                    If the range covers addresses `0x2000` to `0x4000`, then `address_at_offset(0x1500)` would return `Some(0x3500)`."]
                pub fn address_at_offset(&self, offset: usize) -> Option<$address> {
                    if offset <= self.size_in_bytes() {
                        Some(self.start_address() + offset)
                    }
                    else {
                        None
                    }
                }

                #[doc = "Returns a new separate `" $TypeName "` that is extended to include the given [`" $chunk "`]."]
                pub fn to_extended(&self, to_include: $chunk) -> $TypeName {
                    // if the current range was empty, return a new range containing only the given page/frame
                    if self.is_empty() {
                        return $TypeName::new(to_include.clone(), to_include);
                    }
                    let start = core::cmp::min(self.0.start(), &to_include);
                    let end = core::cmp::max(self.0.end(), &to_include);
                    $TypeName::new(start.clone(), end.clone())
                }

                #[doc = "Returns an inclusive `" $TypeName "` representing the [`" $chunk "`]s that overlap \
                    across this `" $TypeName "` and the given other `" $TypeName "`.\n\n \
                    If there is no overlap between the two ranges, `None` is returned."]
                pub fn overlap(&self, other: &$TypeName) -> Option<$TypeName> {
                    let starts = max(*self.start(), *other.start());
                    let ends   = min(*self.end(),   *other.end());
                    if starts <= ends {
                        Some($TypeName::new(starts, ends))
                    } else {
                        None
                    }
                }
            }
            impl fmt::Debug for $TypeName {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "{:?}", self.0)
                }
            }
            impl Deref for $TypeName {
                type Target = RangeInclusive<$chunk>;
                fn deref(&self) -> &RangeInclusive<$chunk> {
                    &self.0
                }
            }
            impl DerefMut for $TypeName {
                fn deref_mut(&mut self) -> &mut RangeInclusive<$chunk> {
                    &mut self.0
                }
            }
            impl IntoIterator for $TypeName {
                type Item = $chunk;
                type IntoIter = RangeInclusive<$chunk>;
                fn into_iter(self) -> Self::IntoIter {
                    self.0
                }
            }
        }
    };
}

implement_page_frame_range!(PageRange, "virtual", virt, Page, VirtualAddress);
implement_page_frame_range!(FrameRange, "physical", phys, Frame, PhysicalAddress);

pub struct FrameAllocator<'a, I> {
    original: I,
    memory_map: I,
    current_descriptor: Option<&'a MemoryDescriptor>,
    next_frame: Frame,
}

impl<'a, I> FrameAllocator<'a, I>
where
    I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    pub fn new(memory_map: I) -> Self {
        // Allocating frames below 0x10000 causes problems during AP startup.
        let start_frame = Frame::containing_address(PhysicalAddress::new_canonical(0x10000));
        Self::new_starting_at(start_frame, memory_map)
    }

    /// Creates a new frame allocator based on the given legacy memory regions.
    /// Skips any frames before the given `frame`.
    pub fn new_starting_at(frame: Frame, memory_map: I) -> Self {
        Self {
            original: memory_map.clone(),
            memory_map,
            current_descriptor: None,
            next_frame: frame,
        }
    }

    fn allocate_frame_from_descriptor(
        &mut self,
        descriptor: &'a MemoryDescriptor,
    ) -> Option<Frame> {
        let start_addr = PhysicalAddress::new_canonical(descriptor.phys_start as usize);
        let start_frame = Frame::containing_address(start_addr);
        let end_addr = start_addr + (descriptor.page_count as usize * PAGE_SIZE);
        let end_frame = Frame::containing_address(end_addr - 1);

        // increase self.next_frame to start_frame if smaller
        if self.next_frame < start_frame {
            self.next_frame = start_frame;
        }

        if self.next_frame < end_frame {
            let ret = self.next_frame;
            self.next_frame += 1;
            Some(ret)
        } else {
            None
        }
    }

    /// Returns the number of memory regions in the underlying memory map.
    ///
    /// The function always returns the same value, i.e. the length doesn't
    /// change after calls to `allocate_frame`.
    pub fn len(&self) -> usize {
        self.original.len()
    }

    /// Returns whether this memory map is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the largest detected physical memory address.
    ///
    /// Useful for creating a mapping for all physical memory.
    pub fn max_phys_addr(&self) -> PhysicalAddress {
        self.original
            .clone()
            .map(|descriptor| {
                PhysicalAddress::new_canonical(
                    descriptor.phys_start as usize + (descriptor.page_count as usize * PAGE_SIZE),
                )
            })
            .max()
            .unwrap()
    }

    pub fn allocate_frame(&mut self) -> Option<Frame> {
        if let Some(current_descriptor) = self.current_descriptor {
            match self.allocate_frame_from_descriptor(current_descriptor) {
                Some(frame) => return Some(frame),
                None => {
                    self.current_descriptor = None;
                }
            }
        }

        // find next suitable descriptor
        while let Some(descriptor) = self.memory_map.next() {
            if descriptor.ty != MemoryType::CONVENTIONAL {
                continue;
            }
            if let Some(frame) = self.allocate_frame_from_descriptor(descriptor) {
                self.current_descriptor = Some(descriptor);
                return Some(frame);
            }
        }

        None
    }
}

pub struct Memory<'a, I> {
    page_allocator: imp::PageAllocator,
    frame_allocator: FrameAllocator<'a, I>,
    mapper: imp::Mapper,
}

impl<'a, I> Memory<'a, I>
where
    I: ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
{
    pub fn new(memory_map: I) -> Self {
        let page_allocator = imp::PageAllocator::new();
        let mut frame_allocator = FrameAllocator::new(memory_map);
        let mapper = imp::Mapper::new(&mut frame_allocator);

        Self {
            page_allocator,
            frame_allocator,
            mapper,
        }
    }

    pub fn get_free_address(&mut self, len: usize) -> VirtualAddress {
        self.page_allocator.get_free_address(len)
    }

    pub fn allocate_frame(&mut self) -> Option<Frame> {
        self.frame_allocator.allocate_frame()
    }

    pub fn map(&mut self, page: Page, frame: Frame, flags: PteFlags) {
        self.mapper
            .map(page, frame, flags, &mut self.frame_allocator);
    }

    pub fn map_segment(&mut self, segment: ProgramHeader, flags: PteFlags) {
        self.page_allocator.mark_segment_as_used(segment);

        let virtual_start = VirtualAddress::new_canonical(segment.p_vaddr as usize);
        let virtual_end_inclusive = virtual_start + segment.p_memsz as usize - 1;

        let physical_start = PhysicalAddress::new_canonical(segment.p_paddr as usize);
        let physical_end_inclusive = physical_start + segment.p_memsz as usize - 1;

        let pages = PageRange::new(
            Page::containing_address(virtual_start),
            Page::containing_address(virtual_end_inclusive),
        )
        .into_iter();
        let frames = FrameRange::new(
            Frame::containing_address(physical_start),
            Frame::containing_address(physical_end_inclusive),
        )
        .into_iter();

        for (page, frame) in pages.zip(frames) {
            self.mapper
                .map(page, frame, flags, &mut self.frame_allocator);
        }
    }
}
