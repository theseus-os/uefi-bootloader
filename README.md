1. Load kernel image
2. Initialise logger/framebuffer
3. Load modules (theseus specific)
4. Get memory map/exit boot services
5. Establish a basic frame allocator
6. Create page tables
7. Find rsdp address?
8. Set up mappings:
    - Parse kernel and load segments
    - Create a stack
        - Guard page
    - Identy map context switch function
    - Create, load, and identy-map GDT
    - Map framebuffer
    - Physical memory offset?
    - Recursive index
9. Create boot info
10. Context switch to kernel
