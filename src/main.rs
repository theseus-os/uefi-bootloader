#![allow(dead_code)]
#![feature(step_trait, abi_efiapi, maybe_uninit_slice, maybe_uninit_write_slice)]
#![no_std]
#![no_main]

mod arch;
mod info;
mod kernel;
mod logger;
mod memory;
mod modules;
mod util;

use crate::{
    info::{FrameBuffer, FrameBufferInfo},
    memory::{Frame, Memory, Page, PhysicalAddress, PteFlags, VirtualAddress},
};
use core::{alloc::Layout, arch::asm, fmt::Write, mem::MaybeUninit, ptr::NonNull, slice};
use info::{BootInformation, ElfSection, MemoryRegion, Module};
use uefi::{
    prelude::entry,
    proto::console::gop::{GraphicsOutput, PixelFormat},
    table::{
        boot::MemoryDescriptor,
        cfg::{ACPI2_GUID, ACPI_GUID},
        Boot, SystemTable,
    },
    Handle, Status,
};

static mut SYSTEM_TABLE: Option<NonNull<SystemTable<Boot>>> = None;

#[entry]
fn main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    let system_table_pointer = NonNull::from(&mut system_table);
    unsafe { SYSTEM_TABLE = Some(system_table_pointer) };

    system_table
        .stdout()
        .clear()
        .expect("failed to clear stdout");

    let frame_buffer = get_frame_buffer(&system_table);
    if let Some(frame_buffer) = frame_buffer {
        init_logger(&frame_buffer);
        log::info!("using framebuffer at {:#x}", frame_buffer.start);
    }

    unsafe { SYSTEM_TABLE = None };

    let rsdp_address = get_rsdp_address(&system_table);

    let mut memory = Memory::new(system_table.boot_services());

    let modules = modules::load(handle, &system_table);
    log::info!("loaded modules");
    let elf_sections = kernel::load(handle, &system_table, &mut memory);
    log::info!("loaded kernel");

    let mappings = set_up_mappings(&mut memory, &frame_buffer);
    log::info!("created memory mappings");

    let page_table = memory.page_table();

    let memory_map_len = system_table.boot_services().memory_map_size().map_size
        + 8 * core::mem::size_of::<MemoryDescriptor>();

    let BootInformationAllocation {
        size: boot_info_size,
        boot_info: boot_info_uninit,
        memory_regions: memory_regions_uninit,
        modules: modules_uninit,
        elf_sections: elf_sections_uninit,
    } = allocate_boot_info(memory, memory_map_len, modules, elf_sections);

    // Zero the bytes so it is sound to interpret the array as initialised.
    unsafe { core::ptr::write_bytes(memory_regions_uninit.as_mut_ptr(), 0, memory_map_len) };
    let (empty_1, memory_regions, empty_2) = unsafe { memory_regions_uninit.align_to_mut::<u8>() };
    assert!(empty_1.is_empty());
    assert!(empty_2.is_empty());

    // At this point memory_regions_uninit has been converted from a
    // &mut [MaybeUninit<MemoryRegion>] to a &mut [u8].

    let memory_regions_count = system_table
        .exit_boot_services(handle, memory_regions)
        .unwrap()
        .1
        .count();

    // Turn the mutable reference into a pointer to adhere to aliasing rules.
    let memory_regions_uninit = memory_regions_uninit.as_mut_ptr();
    // TODO: This is defined behaviour right.
    let memory_regions = unsafe {
        core::slice::from_raw_parts_mut(
            memory_regions_uninit as *mut MemoryRegion,
            memory_regions_count,
        )
    };
    // Now that we've asserted exclusive access over memory_regions, we cannot use
    // this pointer.
    drop(memory_regions_uninit);

    let modules = MaybeUninit::write_slice(modules_uninit, modules);
    let elf_sections = MaybeUninit::write_slice(elf_sections_uninit, elf_sections);

    let boot_info = boot_info_uninit.write(BootInformation {
        size: boot_info_size,
        frame_buffer: mappings.frame_buffer.map(|start| FrameBuffer {
            start: start.value(),
            info: frame_buffer.unwrap().info,
        }),
        rsdp_address,
        memory_regions: memory_regions.into(),
        modules: modules.into(),
        elf_sections: elf_sections.into(),
    });
    log::info!("created boot info");

    log::info!("exited boot services");

    let context = Context {
        page_table,
        stack_top: mappings.stack_top,
        entry_point: todo!(),
        boot_info,
    };

    log::info!("about to switch to kernel: {context:x?}");
    unsafe { context_switch(context) };
}

fn get_frame_buffer(system_table: &SystemTable<Boot>) -> Option<FrameBuffer> {
    let handle = system_table
        .boot_services()
        .get_handle_for_protocol::<GraphicsOutput>()
        .ok()?;
    let mut gop = system_table
        .boot_services()
        .open_protocol_exclusive::<GraphicsOutput>(handle)
        .ok()?;

    let mode_info = gop.current_mode_info();
    let mut frame_buffer = gop.frame_buffer();
    let info = FrameBufferInfo {
        size: frame_buffer.size(),
        width: mode_info.resolution().0,
        height: mode_info.resolution().1,
        pixel_format: match mode_info.pixel_format() {
            PixelFormat::Rgb => info::PixelFormat::Rgb,
            PixelFormat::Bgr => info::PixelFormat::Bgr,
            PixelFormat::Bitmask | PixelFormat::BltOnly => {
                panic!("Bitmask and BltOnly framebuffers are not supported")
            }
        },
        bytes_per_pixel: 4,
        stride: mode_info.stride(),
    };

    Some(FrameBuffer {
        start: frame_buffer.as_mut_ptr() as usize,
        info,
    })
}

fn init_logger(frame_buffer: &FrameBuffer) {
    let slice = unsafe {
        core::slice::from_raw_parts_mut(frame_buffer.start as *mut _, frame_buffer.info.size)
    };
    let logger =
        logger::LOGGER.call_once(move || logger::LockedLogger::new(slice, frame_buffer.info));
    log::set_logger(logger).expect("logger already set");
    log::set_max_level(log::LevelFilter::Trace);
}

fn get_rsdp_address(system_table: &SystemTable<Boot>) -> Option<usize> {
    let mut config_entries = system_table.config_table().iter();
    // look for an ACPI2 RSDP first
    let acpi2_rsdp = config_entries.find(|entry| matches!(entry.guid, ACPI2_GUID));
    // if no ACPI2 RSDP is found, look for a ACPI1 RSDP
    let rsdp = acpi2_rsdp.or_else(|| config_entries.find(|entry| matches!(entry.guid, ACPI_GUID)));
    rsdp.map(|entry| entry.address as usize)
}

fn set_up_mappings<'a, 'b>(
    memory: &'a mut Memory<'b>,
    frame_buffer: &Option<FrameBuffer>,
) -> Mappings {
    // TODO: Reserve kernel frames

    // TODO: enable nxe and write protect bits on x86_64

    // TODO
    const STACK_SIZE: usize = 18 * 4096;

    let stack_start_address = memory.get_free_address(STACK_SIZE);

    let stack_start = Page::containing_address(stack_start_address);
    let stack_end = {
        let end_address = stack_start_address + STACK_SIZE;
        Page::containing_address(end_address - 1)
    };

    // The +1 means the guard page isn't mapped to a frame.
    for page in (stack_start + 1)..=stack_end {
        let frame = memory.allocate_frame().unwrap();
        // TODO: No execute?
        memory.map(page, frame, PteFlags::PRESENT | PteFlags::WRITABLE);
    }

    // TODO: Explain
    memory.map(
        Page::containing_address(VirtualAddress::new_canonical(context_switch as usize)),
        Frame::containing_address(PhysicalAddress::new_canonical(context_switch as usize)),
        PteFlags::PRESENT,
    );

    let frame_buffer = frame_buffer.map(|frame_buffer| {
        let start_virtual = memory.get_free_address(frame_buffer.info.size);

        let start_page = Page::containing_address(start_virtual);
        let end_page = Page::containing_address(start_virtual + frame_buffer.info.size - 1);

        let start_frame =
            Frame::containing_address(PhysicalAddress::new_canonical(frame_buffer.start));
        let end_frame = Frame::containing_address(PhysicalAddress::new_canonical(
            frame_buffer.start + frame_buffer.info.size - 1,
        ));

        for (page, frame) in (start_page..=end_page).zip(start_frame..=end_frame) {
            // We don't need to allocate frames because the frame buffer is already reserved
            // in the memory map.
            memory.map(page, frame, PteFlags::PRESENT | PteFlags::WRITABLE);
        }

        start_virtual
    });

    // TODO: GDT
    // TODO: recursive index

    Mappings {
        stack_top: (stack_end + 1).start_address(),
        frame_buffer,
    }
}

struct Mappings {
    stack_top: VirtualAddress,
    frame_buffer: Option<VirtualAddress>,
}

fn allocate_boot_info<'a, 'b>(
    mut memory: Memory<'b>,
    memory_regions_count: usize,
    modules: &'static [Module],
    elf_sections: &'static [ElfSection],
) -> BootInformationAllocation {
    let boot_info_layout = Layout::new::<BootInformation>();

    let memory_regions_layout = Layout::array::<MemoryRegion>(memory_regions_count).unwrap();
    let (combined, memory_regions_offset) = boot_info_layout.extend(memory_regions_layout).unwrap();

    let modules_layout = Layout::array::<Module>(modules.len()).unwrap();
    let (combined, modules_offset) = combined.extend(modules_layout).unwrap();

    let elf_sections_layout = Layout::array::<ElfSection>(elf_sections.len()).unwrap();
    let (combined, elf_sections_offset) = combined.extend(elf_sections_layout).unwrap();

    let (start_page, end_page) = {
        let boot_info_address = memory.get_free_address(combined.size());
        let elf_sections_end = boot_info_address + combined.size();

        (
            Page::containing_address(boot_info_address),
            Page::containing_address(elf_sections_end - 1),
        )
    };

    // We want to minimise the number of frame allocations to keep
    // num_memory_regions the same.

    let frames = memory
        .allocate_frames((start_page..=end_page).count())
        .unwrap();
    // Abuse UEFI's identy-mapping
    let boot_info_address = frames.start_address();

    for (page, frame) in (start_page..=end_page).zip(frames) {
        memory.map(page, frame, PteFlags::PRESENT | PteFlags::WRITABLE);
    }

    let memory_map_regions_address = boot_info_address + memory_regions_offset;
    let modules_address = boot_info_address + modules_offset;
    let elf_sections_address = boot_info_address + elf_sections_offset;

    let boot_info: &'static mut MaybeUninit<BootInformation> =
        unsafe { &mut *(boot_info_address.value() as *mut _) };
    let memory_regions: &'static mut [MaybeUninit<MemoryRegion>] = unsafe {
        slice::from_raw_parts_mut(
            memory_map_regions_address.value() as *mut _,
            memory_regions_count,
        )
    };
    let modules: &'static mut [MaybeUninit<Module>] =
        unsafe { slice::from_raw_parts_mut(modules_address.value() as *mut _, modules.len()) };
    let elf_sections: &'static mut [MaybeUninit<ElfSection>] = unsafe {
        slice::from_raw_parts_mut(elf_sections_address.value() as *mut _, elf_sections.len())
    };

    BootInformationAllocation {
        size: combined.size(),
        boot_info,
        memory_regions,
        modules,
        elf_sections,
    }
}

struct BootInformationAllocation {
    size: usize,
    boot_info: &'static mut MaybeUninit<BootInformation>,
    memory_regions: &'static mut [MaybeUninit<MemoryRegion>],
    modules: &'static mut [MaybeUninit<Module>],
    elf_sections: &'static mut [MaybeUninit<ElfSection>],
}

unsafe fn context_switch(context: Context) -> ! {
    unsafe {
        asm!(
            "mov cr3, {}; mov rsp, {}; jmp {}",
            in(reg) context.page_table.start_address().value(),
            in(reg) context.stack_top.value(),
            in(reg) context.entry_point.value(),
            in("rdi") context.boot_info as *const _ as usize,
            options(noreturn),
        );
    }
}

#[derive(Debug)]
struct Context {
    page_table: Frame,
    stack_top: VirtualAddress,
    entry_point: VirtualAddress,
    boot_info: &'static mut BootInformation,
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(mut system_table_pointer) = unsafe { SYSTEM_TABLE } {
        let system_table = unsafe { system_table_pointer.as_mut() };
        let _ = writeln!(system_table.stdout(), "{info}");
    }

    if let Some(logger) = logger::LOGGER.get() {
        unsafe { logger.force_unlock() };
    }
    log::error!("{info}");

    loop {
        unsafe { asm!("cli", "hlt") };
    }
}
