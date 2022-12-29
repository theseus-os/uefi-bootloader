use core::mem::MaybeUninit;

use crate::{
    info::ElfSection,
    memory::{Memory, PteFlags},
    util::{allocate_slice, get_file_system_root},
};
use goblin::elf64::{
    header::Header,
    program_header::{ProgramHeader, SIZEOF_PHDR},
    section_header::SIZEOF_SHDR,
};
use uefi::{
    prelude::cstr16,
    proto::media::file::{File, FileAttribute, FileMode, FileType, RegularFile},
    table::{boot::MemoryType, Boot, SystemTable},
    CStr16, Handle,
};

pub fn load<'a, 'b>(
    handle: Handle,
    system_table: &SystemTable<Boot>,
    memory: &'a mut Memory<'b>,
) -> &'static mut [ElfSection] {
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

    let loader = unsafe { Loader::new(kernel_file, memory, system_table) };
    loader.load()
}

struct Loader<'a, 'b, 'c> {
    file: RegularFile,
    memory: &'a mut Memory<'b>,
    system_table: &'c SystemTable<Boot>,
}

impl<'a, 'b, 'c> Loader<'a, 'b, 'c> {
    /// Creates a new loader.
    ///
    /// # Safety
    ///
    /// The file position must be set to the start of the file.
    unsafe fn new(
        file: RegularFile,
        memory: &'a mut Memory<'b>,
        system_table: &'c SystemTable<Boot>,
    ) -> Self {
        Self {
            file,
            memory,
            system_table,
        }
    }

    fn load(mut self) -> &'static mut [ElfSection] {
        let mut buffer = [0; core::mem::size_of::<Header>()];
        self.file.read(&mut buffer).unwrap();

        let kernel_header = Header::from_bytes(&buffer);

        let program_header_offset = kernel_header.e_phoff;
        let program_header_count = kernel_header.e_phnum;

        let mut buffer = [0; SIZEOF_PHDR];

        for i in 0..program_header_count as u64 {
            // Loading segments modifies the file position.
            self.file
                .set_position(program_header_offset + (i * SIZEOF_PHDR as u64))
                .unwrap();
            self.file.read(&mut buffer).unwrap();

            // TODO
            let program_header: ProgramHeader = unsafe { *(buffer.as_ptr() as *mut _) };

            // .got section
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

        self.elf_sections(kernel_header)
    }

    fn elf_sections(&mut self, header: &Header) -> &'static mut [ElfSection] {
        let program_header_count = header.e_shnum;

        let sections = allocate_slice(
            program_header_count as usize,
            MemoryType::LOADER_DATA,
            self.system_table,
        );
        let mut buffer = [0; SIZEOF_SHDR];

        self.file.set_position(header.e_shoff as u64).unwrap();

        for i in 0..program_header_count as usize {
            self.file.read(&mut buffer).unwrap();
            let section_header = unsafe { *(buffer.as_ptr() as *mut _) };
            sections[i].write(section_header);
        }

        unsafe { MaybeUninit::slice_assume_init_mut(sections) }
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
