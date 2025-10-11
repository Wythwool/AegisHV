CC ?= clang
LD ?= ld.lld

CFLAGS := -O2 -std=c11 -ffreestanding -fno-stack-protector -fno-asynchronous-unwind-tables -fno-builtin -Wall -Wextra -Werror -mno-red-zone -m64 -nostdinc -fno-pic
CFLAGS += -I../include -I.
LDFLAGS := -nostdlib -z max-page-size=0x1000

SRC_C := \
  x86/kmain.c \
  common/log.c \
  x86/vmx.c \
  x86/vmcs.c \
  x86/ept.c

SRC_S := \
  x86/boot/mb2_header.S \
  x86/boot/entry64.S

OBJ := $(SRC_C:.c=.o) $(SRC_S:.S=.o)

all: aegishv.elf

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

%.o: %.S
	$(CC) -c $< -o $@

aegishv.elf: $(OBJ) x86/linker.ld
	$(LD) $(LDFLAGS) -T x86/linker.ld -o $@ $(OBJ)

clean:
	rm -f $(OBJ) aegishv.elf

.PHONY: all clean
