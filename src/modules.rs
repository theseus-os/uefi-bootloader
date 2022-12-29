use core::{mem::MaybeUninit, ptr, slice};

use crate::info::Module;
use uefi::{
    prelude::cstr16,
    proto::media::file::{File, FileAttribute, FileMode},
    table::{
        boot::{AllocateType, MemoryType},
        Boot, SystemTable,
    },
    Handle,
};

pub fn load(handle: Handle, system_table: &SystemTable<Boot>) -> &'static mut [Module] {
    let mut root = system_table
        .boot_services()
        .get_image_file_system(handle)
        .unwrap()
        .open_volume()
        .unwrap();

    let mut dir = root
        .open(cstr16!("modules"), FileMode::Read, FileAttribute::empty())
        .unwrap()
        .into_directory()
        .unwrap();

    let mut num_modules = 0;
    let mut num_pages = 0;
    let mut buf = [0; 500];

    while let Some(info) = dir.read_entry(&mut buf).unwrap() {
        if !info.attribute().contains(FileAttribute::DIRECTORY) {
            num_modules += 1;
            // Theseus modules must not share pages i.e. the next module starts on a new
            // page. TODO: Ideally we'd remove this constraint.
            num_pages += calculate_pages(info.file_size() as usize);
        }
    }

    let modules = allocate_slice(num_modules, MemoryType::LOADER_DATA, system_table);
    let raw_bytes = allocate_slice(
        num_pages * 4096,
        MemoryType::custom(0x80000000),
        system_table,
    );
    // SAFETY: allocate_slice zeroed the bytes so they are initialised.
    let raw_bytes = unsafe { MaybeUninit::slice_assume_init_mut(raw_bytes) };

    dir.reset_entry_readout().unwrap();

    let mut idx = 0;
    let mut num_pages = 0;

    while let Some(info) = dir.read_entry(&mut buf).unwrap() {
        if !info.attribute().contains(FileAttribute::DIRECTORY) {
            let name = info.file_name();

            let len = info.file_size() as usize;
            let mut file = dir
                .open(info.file_name(), FileMode::Read, FileAttribute::empty())
                .unwrap()
                .into_regular_file()
                .unwrap();

            file.read(&mut raw_bytes[(num_pages * 4096)..]).unwrap();

            let mut name_buf = [0; 64];
            let mut name_idx = 0;
            for c16 in name.iter() {
                let c = char::from(*c16);
                let s = c.encode_utf8(&mut name_buf[name_idx..(name_idx + 4)]);
                name_idx += s.len();
            }

            modules[idx].write(Module {
                name: name_buf,
                offset: num_pages * 4096,
                len,
            });

            idx += 1;
            num_pages += calculate_pages(len);
        }
    }

    assert_eq!(idx, modules.len());
    unsafe { MaybeUninit::slice_assume_init_mut(modules) }
}

fn allocate_slice<T>(
    len: usize,
    ty: MemoryType,
    st: &SystemTable<Boot>,
) -> &'static mut [MaybeUninit<T>] {
    let bytes_len = core::mem::size_of::<T>() * len;
    let num_pages = calculate_pages(bytes_len);
    let pointer = st
        .boot_services()
        .allocate_pages(AllocateType::AnyPages, ty, num_pages)
        .unwrap() as *mut _;
    unsafe { ptr::write_bytes(pointer, 0, len) };
    let slice = unsafe { slice::from_raw_parts_mut(pointer, len) };
    slice
}

fn calculate_pages(bytes: usize) -> usize {
    ((bytes - 1) / 4096) + 1
}
