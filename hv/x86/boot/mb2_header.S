.section .multiboot2
.align 8
multiboot2_header:
    /* magic + architecture (0 for i386) */
    .long 0xe85250d6
    .long 0
    /* length of header */
    .long (multiboot2_end - multiboot2_header)
    /* checksum */
    .long -(0xe85250d6 + 0 + (multiboot2_end - multiboot2_header))

    /* end tag */
    .short 0
    .short 0
    .long 8

multiboot2_end:
