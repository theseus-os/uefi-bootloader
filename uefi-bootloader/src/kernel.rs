use crate::{memory::VirtualAddress, BootContext};
use core::mem::MaybeUninit;
use goblin::elf64::{
    header::Header,
    program_header::{ProgramHeader, SIZEOF_PHDR},
    section_header::{SectionHeader, SIZEOF_SHDR},
};
use uefi::{
    prelude::cstr16,
    proto::media::file::{File, FileAttribute, FileMode, FileType, RegularFile},
    table::boot::MemoryType,
    CStr16,
};
use uefi_bootloader_api::ElfSection;

impl BootContext {
    pub(crate) fn load_kernel(&mut self) -> (VirtualAddress, &'static mut [ElfSection]) {
        let mut root = self.open_file_system_root().unwrap();

        const KERNEL_NAME: &CStr16 = cstr16!("kernel.elf");

        let file = match root
            .open(KERNEL_NAME, FileMode::Read, FileAttribute::empty())
            .expect("failed to load kernel")
            .into_type()
            .unwrap()
        {
            FileType::Regular(file) => file,
            FileType::Dir(_) => panic!(),
        };

        Loader {
            file,
            context: self,
        }
        .load()
    }
}

struct Loader<'a> {
    file: RegularFile,
    context: &'a mut BootContext,
}

impl<'a> Loader<'a> {
    fn load(mut self) -> (VirtualAddress, &'static mut [ElfSection]) {
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

            // TODO: Is there a neater way of doing this?
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
                7 => {}
                // Probably GNU_STACK
                // TODO: Remove from nano_core binary?
                _ => {}
            }
        }

        (
            VirtualAddress::new_canonical(kernel_header.e_entry as usize),
            self.elf_sections(kernel_header),
        )
    }

    fn elf_sections(&mut self, header: &Header) -> &'static mut [ElfSection] {
        let program_header_count = header.e_shnum;

        let sections = self
            .context
            .allocate_slice(program_header_count as usize, MemoryType::LOADER_DATA);
        let mut buffer = [0; SIZEOF_SHDR];

        let shstrtab_header = header.e_shoff + (header.e_shstrndx as u64 * SIZEOF_SHDR as u64);
        self.file.set_position(shstrtab_header).unwrap();
        self.file.read(&mut buffer).unwrap();
        let shstrtab_section_header: SectionHeader = unsafe { *(buffer.as_ptr() as *mut _) };
        let shstrtab_base = shstrtab_section_header.sh_offset;

        for (i, uninit_section) in sections.iter_mut().enumerate() {
            self.file
                .set_position(header.e_shoff + (i * SIZEOF_SHDR) as u64)
                .unwrap();
            self.file.read(&mut buffer).unwrap();
            let section_header: SectionHeader = unsafe { *(buffer.as_ptr() as *mut _) };

            let mut name = [0; 64];
            let name_position = shstrtab_base + section_header.sh_name as u64;
            self.file.set_position(name_position).unwrap();
            self.file.read(&mut name).unwrap();

            uninit_section.write(ElfSection {
                name,
                start: section_header.sh_addr as usize,
                size: section_header.sh_size as usize,
                flags: section_header.sh_flags,
            });
        }

        unsafe { MaybeUninit::slice_assume_init_mut(sections) }
    }

    fn handle_load_segment(&mut self, segment: ProgramHeader) {
        log::info!("loading segment: {segment:?}");
        let slice = unsafe { self.context.map_segment(segment) };

        self.file.set_position(segment.p_offset).unwrap();
        self.file
            .read(&mut slice[..segment.p_filesz as usize])
            .unwrap();

        // let bss_start = (segment.p_paddr + segment.p_filesz) as *mut u8;
        // let bss_size = (segment.p_memsz - segment.p_filesz) as usize;

        // unsafe { core::ptr::write_bytes(bss_start, 0, bss_size) }
    }
}
