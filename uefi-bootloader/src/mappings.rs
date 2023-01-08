use crate::{
    context_switch,
    memory::{Frame, FrameAllocator, Page, PhysicalAddress, PteFlags, VirtualAddress},
    RuntimeContext,
};

impl RuntimeContext {
    pub(crate) fn set_up_mappings(&mut self) -> VirtualAddress {
        // TODO: Enable nxe and write protect bits on x86_64.

        // TODO: Depend on kernel_config?
        const STACK_SIZE: usize = 18 * 4096;

        let stack_start_address = self.page_allocator.get_free_address(STACK_SIZE);

        let stack_start = Page::containing_address(stack_start_address);
        let stack_end = {
            let end_address = stack_start_address + STACK_SIZE;
            Page::containing_address(end_address - 1)
        };

        // The +1 means the guard page isn't mapped to a frame.
        for page in (stack_start + 1)..=stack_end {
            let frame = self
                .frame_allocator
                .allocate_frame()
                .expect("failed to allocate stack frame");
            self.mapper.map(
                page,
                frame,
                PteFlags::new()
                    .present(true)
                    .writable(true)
                    .no_execute(true),
                &mut self.frame_allocator,
            );
        }

        // Identity-map the context switch function so that when it switches to the new
        // page table, it continues executing.
        self.mapper.map(
            Page::containing_address(VirtualAddress::new_canonical(context_switch as usize)),
            Frame::containing_address(PhysicalAddress::new_canonical(context_switch as usize)),
            PteFlags::new().present(true),
            &mut self.frame_allocator,
        );

        crate::memory::set_up_arch_specific_mappings(self);

        (stack_end + 1).start_address()
    }
}
