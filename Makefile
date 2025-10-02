ISO=aegishv.iso
ELF=dist/kernel.elf
BIN_DIR=dist
NASM=nasm
NASMFLAGS=-f elf64 -Wall -O2
LD=ld
LDFLAGS=-T kernel.ld -nostdlib

SRC=src/boot.asm src/gdt.asm src/vmx.asm
OBJS=$(SRC:.asm=.o)

all: $(ELF)

dist:
	mkdir -p dist

%.o: %.asm
	$(NASM) $(NASMFLAGS) -o $@ $<

$(ELF): dist $(OBJS) kernel.ld
	$(LD) $(LDFLAGS) -o $(ELF) $(OBJS)

iso: all
	rm -rf isoroot
	mkdir -p isoroot/boot/grub
	cp $(ELF) isoroot/boot/kernel.elf
	cp grub/grub.cfg isoroot/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO) isoroot

clean:
	rm -rf dist isoroot $(ISO) src/*.o

.PHONY: all iso clean
