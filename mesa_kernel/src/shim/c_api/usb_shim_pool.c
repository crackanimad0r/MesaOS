/*
 * usb_shim_pool.c - Pool DMA y allocator del data_pool compartido
 *
 * License: MIT
 */

#include "usb_shim_core.h"
#include <string.h>

/* ══════════════════════════════════════════════════════════════════════════
 * Pool DMA (para tablas xHCI: segmentos, contextos, etc.)
 * Alineado a 64 bytes como requiere el xHCI spec.
 * ══════════════════════════════════════════════════════════════════════════ */

static uint8_t dma_pool[DMA_POOL_SIZE] __attribute__((aligned(4096)));
static uint32_t dma_pool_offset = 0;

void *shim_dma_alloc(size_t size, uint64_t *phys_out) {
    size = (size + 63) & ~63;
    uint32_t off = __sync_fetch_and_add(&dma_pool_offset, (uint32_t)size);
    if (off + size > DMA_POOL_SIZE) return NULL;
    memset(&dma_pool[off], 0, size);
    if (phys_out) *phys_out = (uint64_t)(uintptr_t)&dma_pool[off];
    return &dma_pool[off];
}

/* ══════════════════════════════════════════════════════════════════════════
 * Pool de datos compartido (bump allocator, se resetea por ciclo)
 * ══════════════════════════════════════════════════════════════════════════ */

static uint32_t pool_offset = 0;

int shim_data_pool_alloc(struct shim_region *region, size_t size) {
    (void)region;
    size = (size + 7) & ~7;
    uint32_t off = __sync_fetch_and_add(&pool_offset, (uint32_t)size);
    if (off + size > SHIM_DATA_POOL_SIZE) return -1;
    return (int)off;
}

void shim_data_pool_free(struct shim_region *region, int offset) {
    (void)region; (void)offset;
}

/* ══════════════════════════════════════════════════════════════════════════
 * Colas SCM (lock-free SPSC sobre memoria compartida)
 * ══════════════════════════════════════════════════════════════════════════ */

/* Comandos: kernel produce, shim consume */
int scm_queue_push(struct scm_queue *q, const struct scm_command *cmd) {
    uint32_t head = __atomic_load_n(&q->head, __ATOMIC_ACQUIRE);
    uint32_t tail = __atomic_load_n(&q->tail, __ATOMIC_ACQUIRE);
    uint32_t next = (head + 1) % SCM_QUEUE_DEPTH;
    if (next == tail) return -1;
    memcpy((void *)&q->entries[head], cmd, sizeof(struct scm_command));
    __atomic_thread_fence(__ATOMIC_RELEASE);
    __atomic_store_n(&q->head, next, __ATOMIC_RELEASE);
    return 0;
}

int scm_queue_pop(struct scm_queue *q, struct scm_command *cmd) {
    uint32_t head = __atomic_load_n(&q->head, __ATOMIC_ACQUIRE);
    uint32_t tail = __atomic_load_n(&q->tail, __ATOMIC_ACQUIRE);
    if (head == tail) return -1;
    memcpy(cmd, (void *)&q->entries[tail], sizeof(struct scm_command));
    __atomic_thread_fence(__ATOMIC_ACQUIRE);
    __atomic_store_n(&q->tail, (tail + 1) % SCM_QUEUE_DEPTH, __ATOMIC_RELEASE);
    return 0;
}

/* Eventos: shim produce, kernel consume.
 * Usamos type-punning: el queue almacena scm_command pero escribimos scm_event.
 * scm_event (32B) cabe en scm_command (56B), así que es seguro. */
int scm_event_queue_push(struct scm_queue *q, const struct scm_event *evt) {
    uint32_t head = __atomic_load_n(&q->head, __ATOMIC_ACQUIRE);
    uint32_t tail = __atomic_load_n(&q->tail, __ATOMIC_ACQUIRE);
    uint32_t next = (head + 1) % SCM_QUEUE_DEPTH;
    if (next == tail) return -1;
    volatile struct scm_event *slot = (volatile struct scm_event *)&q->entries[head];
    slot->type      = evt->type;
    slot->id        = evt->id;
    slot->status    = evt->status;
    slot->actual_len = evt->actual_len;
    slot->data_ofs  = evt->data_ofs;
    slot->data_len  = evt->data_len;
    slot->reserved  = evt->reserved;
    __atomic_thread_fence(__ATOMIC_RELEASE);
    __atomic_store_n(&q->head, next, __ATOMIC_RELEASE);
    return 0;
}
