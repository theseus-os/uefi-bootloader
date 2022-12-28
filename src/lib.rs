#![allow(dead_code)]
#![feature(step_trait, abi_efiapi)]
#![no_std]

mod arch;
mod info;
mod logger;
mod memory;

use crate::{
    info::{FrameBuffer, FrameBufferInfo},
    memory::FrameAllocator,
};
use goblin::elf64::{header::Header, program_header::ProgramHeader};
use uefi::{
    prelude::{cstr16, entry, Boot, SystemTable},
    proto::{
        console::gop::{GraphicsOutput, PixelFormat},
        media::file::{File, FileAttribute, FileMode, FileType, RegularFile},
    },
    table::boot::{MemoryDescriptor, MemoryType},
    CStr16, Handle, Status,
};

#[entry]
fn efi_main(handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    system_table
        .stdout()
        .clear()
        .expect("failed to clear stdout");

    let frame_buffer = get_frame_buffer(handle, &system_table);
    if let Some(frame_buffer) = frame_buffer {
        init_logger(&frame_buffer);
        log::info!("Using framebuffer at {:#x}", frame_buffer.start);
    }

    let memory_map_buffer = {
        let memory_map_size = system_table.boot_services().memory_map_size().map_size
            + 8 * core::mem::size_of::<MemoryDescriptor>();
        let pointer = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, memory_map_size)
            .unwrap();
        unsafe { core::slice::from_raw_parts_mut(pointer, memory_map_size) }
    };
    let (_, memory_map) = system_table
        .boot_services()
        .memory_map(memory_map_buffer)
        .unwrap();

    let _frame_allocator = FrameAllocator::new(memory_map);

    todo!();
}

fn get_frame_buffer(handle: Handle, system_table: &SystemTable<Boot>) -> Option<FrameBuffer> {
    let mut gop = system_table
        .boot_services()
        .open_protocol_exclusive::<GraphicsOutput>(handle)
        .ok()?;

    let mode_info = gop.current_mode_info();
    let mut frame_buffer = gop.frame_buffer();
    let info = FrameBufferInfo {
        len: frame_buffer.size(),
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
        core::slice::from_raw_parts_mut(frame_buffer.start as *mut _, frame_buffer.info.len)
    };
    let logger =
        logger::LOGGER.call_once(move || logger::LockedLogger::new(slice, frame_buffer.info));
    log::set_logger(logger).expect("logger already set");
    log::set_max_level(log::LevelFilter::Trace);
}

fn load_kernel(handle: Handle, system_table: &SystemTable<Boot>) -> Option<()> {
    let mut root = system_table
        .boot_services()
        .get_image_file_system(handle)
        .ok()?
        .open_volume()
        .ok()?;

    const KERNEL_NAME: &CStr16 = cstr16!("kernel.elf");

    let mut kernel_file = match root
        .open(KERNEL_NAME, FileMode::Read, FileAttribute::empty())
        .expect("failed to load kernel")
        .into_type()
        .unwrap()
    {
        FileType::Regular(file) => file,
        FileType::Dir(_) => panic!(),
    };

    // TODO: Smaller buffer?
    let mut buffer = [0; core::mem::size_of::<Header>()];
    kernel_file.read(&mut buffer).unwrap();

    let kernel_header = Header::from_bytes(&buffer);

    let program_header_offset = kernel_header.e_phoff;
    let program_header_count = kernel_header.e_phnum;

    const PROGRAM_HEADER_SIZE: usize = 0x38;
    assert_eq!(kernel_header.e_phentsize as usize, PROGRAM_HEADER_SIZE);
    assert_eq!(
        kernel_header.e_ehsize as usize,
        core::mem::size_of::<Header>()
    );

    let mut buffer = [0; PROGRAM_HEADER_SIZE];
    kernel_file.set_position(program_header_offset).unwrap();

    for _ in 0..program_header_count {
        kernel_file.read(&mut buffer).unwrap();

        // TODO
        assert_eq!(
            core::mem::size_of_val(&buffer),
            core::mem::size_of::<ProgramHeader>(),
        );
        let program_header: ProgramHeader = unsafe { *(buffer.as_ptr() as *mut _) };

        match program_header.p_type {
            // Loadable
            1 => handle_load_segment(&kernel_file, program_header),
            // TLS
            7 => todo!(),
            _ => todo!(),
        }
    }

    Some(())
}

fn handle_load_segment(_file: &RegularFile, header: ProgramHeader) {
    let _physical_start = header.p_paddr;
    todo!();
}
