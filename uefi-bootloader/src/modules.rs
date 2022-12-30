use crate::util::{allocate_slice, calculate_pages, get_file_system_root};
use core::mem::MaybeUninit;
use uefi::{
    prelude::cstr16,
    proto::media::file::{File, FileAttribute, FileMode},
    table::{boot::MemoryType, Boot, SystemTable},
    Handle,
};
use uefi_bootloader_api::Module;

pub fn load(handle: Handle, system_table: &SystemTable<Boot>) -> &'static mut [Module] {
    let mut root = get_file_system_root(handle, system_table).unwrap();

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
            log::info!("{name_buf:?}");

            idx += 1;
            num_pages += calculate_pages(len);
        }
    }

    assert_eq!(idx, modules.len());
    unsafe { MaybeUninit::slice_assume_init_mut(modules) }
}
