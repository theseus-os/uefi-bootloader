use crate::{
    memory::{Memory, PteFlags},
    util::get_file_system_root,
};
use goblin::elf64::{header::Header, program_header::ProgramHeader};
use uefi::{
    prelude::cstr16,
    proto::media::file::{File, FileAttribute, FileMode, FileType, RegularFile},
    table::{boot::MemoryDescriptor, Boot, SystemTable},
    CStr16, Handle,
};

pub fn load<'a, 'b>(handle: Handle, system_table: &SystemTable<Boot>, memory: &'a mut Memory<'b>) {
    let mut root = get_file_system_root(handle, system_table).unwrap();

    const KERNEL_NAME: &CStr16 = cstr16!("kernel.elf");

    let kernel_file = match root
        .open(KERNEL_NAME, FileMode::Read, FileAttribute::empty())
        .expect("failed to load kernel")
        .into_type()
        .unwrap()
    {
        FileType::Regular(file) => file,
        FileType::Dir(_) => panic!(),
    };

    let loader = unsafe { Loader::new(kernel_file, memory) };
    loader.load();
}

struct Loader<'a, 'b> {
    file: RegularFile,
    memory: &'a mut Memory<'b>,
}

impl<'a, 'b> Loader<'a, 'b> {
    /// Creates a new loader.
    ///
    /// # Safety
    ///
    /// The file position must be set to the start of the file.
    unsafe fn new(file: RegularFile, memory: &'a mut Memory<'b>) -> Self {
        Self { file, memory }
    }

    fn load(mut self) {
        let mut buffer = [0; core::mem::size_of::<Header>()];
        self.file.read(&mut buffer).unwrap();

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

        for i in 0..program_header_count as u64 {
            // Loading segments modifies the file position.
            self.file
                .set_position(
                    program_header_offset + (i * core::mem::size_of::<ProgramHeader>() as u64),
                )
                .unwrap();
            self.file.read(&mut buffer).unwrap();

            // TODO
            assert_eq!(
                core::mem::size_of_val(&buffer),
                core::mem::size_of::<ProgramHeader>(),
            );
            let program_header: ProgramHeader = unsafe { *(buffer.as_ptr() as *mut _) };

            if program_header.p_memsz == 0 {
                continue;
            }

            match program_header.p_type {
                0 => {}
                // Loadable
                1 => self.handle_load_segment(program_header),
                // TLS
                // TODO?
                7 => {}
                // Probably GNU_STACK
                // TODO: Remove from nano_core binary?
                _ => {}
            }
        }
    }

    fn handle_load_segment(&mut self, segment: ProgramHeader) {
        log::info!("handling load segment: {segment:?}");
        let mut flags = PteFlags::PRESENT;

        // If the first bit isn't set
        if segment.p_flags & 0x1 == 0 {
            flags |= PteFlags::NO_EXECUTE;
        }

        // If the second bit is set
        if segment.p_flags & 0x2 != 0 {
            flags |= PteFlags::WRITABLE;
        }

        self.memory.map_segment(segment, flags);

        let slice = unsafe {
            core::slice::from_raw_parts_mut(segment.p_paddr as *mut u8, segment.p_filesz as usize)
        };

        self.file.set_position(segment.p_offset).unwrap();
        // FIXME: We don't check that the physical address is safe to write to. But, if
        // it isn't, there isn't much we can do because Theseus requires kernel segment
        // load addresses to be respected.
        self.file.read(slice).unwrap();

        let bss_start = (segment.p_offset + segment.p_filesz) as *mut u8;
        let bss_size = (segment.p_memsz - segment.p_filesz) as usize;

        unsafe { core::ptr::write_bytes(bss_start, 0, bss_size) }
    }
}
