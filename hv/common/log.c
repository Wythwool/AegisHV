#include <stdint.h>
#include "log.h"

static inline void outb(uint16_t p, uint8_t v){ __asm__ volatile("outb %0,%1"::"a"(v),"Nd"(p)); }
static inline uint8_t inb(uint16_t p){ uint8_t v; __asm__ volatile("inb %1,%0":"=a"(v):"Nd"(p)); return v; }

static void serial_init(void){
    outb(0x3F8 + 1, 0x00);
    outb(0x3F8 + 3, 0x80);
    outb(0x3F8 + 0, 0x01);
    outb(0x3F8 + 1, 0x00);
    outb(0x3F8 + 3, 0x03);
    outb(0x3F8 + 2, 0xC7);
    outb(0x3F8 + 4, 0x0B);
}

static void serial_putc(char c){
    while(!(inb(0x3F8 + 5) & 0x20)) { }
    outb(0x3F8, (uint8_t)c);
    __asm__ volatile("outb %0, %1"::"a"(c),"Nd"(0xE9)); // bochs
}

static void serial_print(const char* s){
    for(; *s; ++s){
        if(*s=='\n') serial_putc('\r');
        serial_putc(*s);
    }
}

void hv_log(const char* level, const char* msg){
    static int inited = 0;
    if(!inited){ serial_init(); inited=1; }
    serial_print("[");
    serial_print(level);
    serial_print("] ");
    serial_print(msg);
    serial_print("\n");
}
