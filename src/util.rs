use core::mem::MaybeUninit;
use uefi::{
    proto::{
        device_path::DevicePath,
        loaded_image::LoadedImage,
        media::{file::Directory, fs::SimpleFileSystem},
    },
    table::{
        boot::{AllocateType, MemoryType},
        Boot, SystemTable,
    },
    Handle,
};

pub(crate) fn get_file_system_root(
    image_handle: Handle,
    system_table: &SystemTable<Boot>,
) -> Option<Directory> {
    let boot_services = system_table.boot_services();

    let loaded_image = boot_services
        .open_protocol_exclusive::<LoadedImage>(image_handle)
        .ok()?;
    let device_path = boot_services
        .open_protocol_exclusive::<DevicePath>(loaded_image.device())
        .ok()?;
    let device_handle = boot_services
        .locate_device_path::<SimpleFileSystem>(&mut &*device_path)
        .ok()?;
    boot_services
        .open_protocol_exclusive::<SimpleFileSystem>(device_handle)
        .ok()?
        .open_volume()
        .ok()
}

pub(crate) fn allocate_slice<T>(
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
    unsafe { core::ptr::write_bytes(pointer, 0, len) };
    let slice = unsafe { core::slice::from_raw_parts_mut(pointer, len) };
    slice
}

pub(crate) fn calculate_pages(bytes: usize) -> usize {
    ((bytes - 1) / 4096) + 1
}
