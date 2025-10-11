#pragma once
#include <stdint.h>
#include <stdatomic.h>

typedef struct {
    atomic_uint write_idx;
    atomic_uint read_idx;
    uint32_t    capacity;
    uint8_t*    data; // fixed-size records of record_size
    uint32_t    record_size;
} ringbuf_t;

static inline void ringbuf_init(ringbuf_t* rb, void* mem, uint32_t cap, uint32_t rec_sz) {
    rb->capacity = cap;
    rb->data = (uint8_t*)mem;
    rb->record_size = rec_sz;
    atomic_store(&rb->write_idx, 0);
    atomic_store(&rb->read_idx, 0);
}

// single-producer, single-consumer
static inline int ringbuf_push(ringbuf_t* rb, const void* rec) {
    uint32_t w = atomic_load_explicit(&rb->write_idx, memory_order_relaxed);
    uint32_t r = atomic_load_explicit(&rb->read_idx, memory_order_acquire);
    if (((w + 1) % rb->capacity) == r) return -1; // full
    uint8_t* dst = rb->data + (w * rb->record_size);
    for (uint32_t i = 0; i < rb->record_size; ++i) dst[i] = ((const uint8_t*)rec)[i];
    atomic_store_explicit(&rb->write_idx, (w + 1) % rb->capacity, memory_order_release);
    return 0;
}

static inline int ringbuf_pop(ringbuf_t* rb, void* out) {
    uint32_t r = atomic_load_explicit(&rb->read_idx, memory_order_relaxed);
    uint32_t w = atomic_load_explicit(&rb->write_idx, memory_order_acquire);
    if (r == w) return -1; // empty
    uint8_t* src = rb->data + (r * rb->record_size);
    for (uint32_t i = 0; i < rb->record_size; ++i) ((uint8_t*)out)[i] = src[i];
    atomic_store_explicit(&rb->read_idx, (r + 1) % rb->capacity, memory_order_release);
    return 0;
}
