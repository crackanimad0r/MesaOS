/*
 * usb_shim_xhci.c - Inicialización del controlador xHCI
 *
 * License: MIT
 */

#include "usb_shim_core.h"

/* ══════════════════════════════════════════════════════════════════════════
 * Command Ring
 * ══════════════════════════════════════════════════════════════════════════ */

static int cmd_ring_init(struct usb_shim_context *ctx) {
    ctx->cmd_ring_seg = (struct xhci_seg *)shim_dma_alloc(
        sizeof(struct xhci_seg), &ctx->cmd_ring_dma);
    if (!ctx->cmd_ring_seg) return -1;

    ctx->cmd_ring_enq_idx = 0;
    ctx->cmd_ring_cycle = 1;

    struct xhci_trb *link = &ctx->cmd_ring_seg->trbs[255];
    link->parameter = ctx->cmd_ring_dma;
    link->status = 0;
    link->control = (TRB_LINK << 10) | TRB_TC | TRB_C;

    uint64_t crcr = ctx->cmd_ring_dma | (ctx->cmd_ring_cycle ? 1 : 0);
    mmio_write64((volatile void *)(ctx->op_base + XHCI_CRCR), crcr);
    return 0;
}

int cmd_ring_enqueue(struct usb_shim_context *ctx, struct xhci_trb *trb) {
    if (ctx->cmd_ring_enq_idx >= 255) return -1;

    struct xhci_trb *slot = &ctx->cmd_ring_seg->trbs[ctx->cmd_ring_enq_idx];
    trb->control |= (ctx->cmd_ring_cycle ? TRB_C : 0);
    slot->parameter = trb->parameter;
    slot->status    = trb->status;
    slot->control   = trb->control;

    __sync_synchronize();

    ctx->cmd_ring_enq_idx = (ctx->cmd_ring_enq_idx + 1) % 256;
    if (ctx->cmd_ring_enq_idx == 255) {
        ctx->cmd_ring_enq_idx = 0;
        ctx->cmd_ring_cycle ^= 1;
    }

    mmio_write32(ctx->doorbells, 0); /* Doorbell 0 = command */
    return 0;
}

/* ══════════════════════════════════════════════════════════════════════════
 * Event Ring
 * ══════════════════════════════════════════════════════════════════════════ */

static int evt_ring_init(struct usb_shim_context *ctx) {
    ctx->evt_ring_seg = (struct xhci_seg *)shim_dma_alloc(
        sizeof(struct xhci_seg), &ctx->evt_ring_dma);
    if (!ctx->evt_ring_seg) return -1;

    ctx->evt_ring_deq_idx = 0;
    ctx->evt_ring_cycle = 1;

    ctx->erst = (struct xhci_erst_entry *)shim_dma_alloc(
        sizeof(struct xhci_erst_entry), &ctx->erst_dma);
    if (!ctx->erst) return -1;

    ctx->erst[0].seg_addr = ctx->evt_ring_dma;
    ctx->erst[0].seg_size = 256;

    uint32_t irq_off = 0x20; /* IRQ0 set */
    mmio_write32((volatile void *)(ctx->rt_base + irq_off + 0x08), 1);
    mmio_write64((volatile void *)(ctx->rt_base + irq_off + 0x10), ctx->erst_dma);
    mmio_write64((volatile void *)(ctx->rt_base + irq_off + 0x18),
                 ctx->evt_ring_dma | 8ULL);
    return 0;
}

/* Procesa TRBs de eventos completados (llamado desde shim_poll_irq) */
void evt_ring_process(struct usb_shim_context *ctx) {
    while (1) {
        struct xhci_trb *evt = &ctx->evt_ring_seg->trbs[ctx->evt_ring_deq_idx];
        uint32_t cycle_bit = evt->control & TRB_C;
        uint32_t expected  = ctx->evt_ring_cycle ? TRB_C : 0;
        if (cycle_bit != expected) break;

        uint8_t trb_type   = (evt->control >> 10) & 0x3F;
        uint32_t compl_code = (evt->status >> 24) & 0xFF;
        uint32_t slot_id    = (evt->status >> 16) & 0xFF;

        struct scm_event scm_evt;
        scm_evt.type       = EVT_URB_COMPLETE;
        scm_evt.id         = (uint32_t)(evt->parameter & 0xFFFFFFFF);
        scm_evt.status     = (compl_code == TRB_SUCCESS || compl_code == TRB_SHORT_PACKET)
                             ? SCM_OK : -EIO;
        scm_evt.actual_len = evt->status & 0xFFFF;
        scm_evt.data_ofs   = slot_id;
        scm_evt.data_len   = 0;
        scm_evt.reserved   = 0;

        if (trb_type >= 32 && trb_type <= 33) {
            scm_event_queue_push(&ctx->region->evt_queue, &scm_evt);
        } else if (trb_type >= 9 && trb_type <= 17) {
            scm_event_queue_push(&ctx->region->evt_queue, &scm_evt);
        }

        ctx->evt_ring_deq_idx = (ctx->evt_ring_deq_idx + 1) % 256;
        if (ctx->evt_ring_deq_idx == 0) ctx->evt_ring_cycle ^= 1;

        uint64_t erdp = (ctx->evt_ring_dma +
            (uint64_t)ctx->evt_ring_deq_idx * sizeof(struct xhci_trb)) |
            (ctx->evt_ring_cycle ? 8ULL : 0ULL);
        mmio_write64((volatile void *)(ctx->rt_base + 0x20 + 0x18), erdp);
    }
}

/* ══════════════════════════════════════════════════════════════════════════
 * Reset + Start HC
 * ══════════════════════════════════════════════════════════════════════════ */

static int xhci_reset(struct usb_shim_context *ctx) {
    mmio_write32((volatile void *)(ctx->op_base + XHCI_USBCMD), XHCI_CMD_HCRST);
    for (int t = 100000; t; t--) {
        if (!(mmio_read32((volatile void *)(ctx->op_base + XHCI_USBSTS)) & XHCI_STS_CNR))
            return 0;
        for (volatile int i = 0; i < 100; i++);
    }
    return -1;
}

static int xhci_start(struct usb_shim_context *ctx) {
    mmio_write32((volatile void *)(ctx->op_base + XHCI_USBCMD), XHCI_CMD_RUN);
    for (int t = 100000; t; t--) {
        if (!(mmio_read32((volatile void *)(ctx->op_base + XHCI_USBSTS)) & XHCI_STS_HCH))
            return 0;
        for (volatile int i = 0; i < 100; i++);
    }
    return -1;
}

/* ══════════════════════════════════════════════════════════════════════════
 * xHCI Init (punto de entrada único)
 * ══════════════════════════════════════════════════════════════════════════ */

int xhci_init(struct usb_shim_context *ctx, uint64_t mmio_phys, uint64_t mmio_size) {
    ctx->mmio_base = (volatile uint8_t *)(uintptr_t)mmio_phys;
    ctx->mmio_size = mmio_size;

    ctx->caplength   = mmio_read32((volatile void *)ctx->mmio_base) & 0xFF;
    ctx->hcs_params1 = mmio_read32((volatile void *)(ctx->mmio_base + XHCI_HCSPARAMS1));
    ctx->hcs_params2 = mmio_read32((volatile void *)(ctx->mmio_base + XHCI_HCSPARAMS2));
    ctx->hcs_params3 = mmio_read32((volatile void *)(ctx->mmio_base + XHCI_HCSPARAMS3));
    ctx->hcc_params1 = mmio_read32((volatile void *)(ctx->mmio_base + XHCI_HCCPARAMS1));

    ctx->max_slots = (ctx->hcs_params1 >> 0) & 0xFF;
    ctx->max_intrs = (ctx->hcs_params1 >> 8) & 0x3FF;
    ctx->max_ports = (ctx->hcs_params1 >> 24) & 0xFF;

    uint32_t db_off = mmio_read32((volatile void *)(ctx->mmio_base + XHCI_DBOFF));
    uint32_t rt_off = mmio_read32((volatile void *)(ctx->mmio_base + XHCI_RTSOFF));

    ctx->op_base   = ctx->mmio_base + ctx->caplength;
    ctx->db_base   = ctx->mmio_base + (db_off & ~1);
    ctx->rt_base   = ctx->mmio_base + rt_off;
    ctx->doorbells = (volatile uint32_t *)ctx->db_base;
    ctx->page_size = mmio_read32((volatile void *)(ctx->op_base + XHCI_PAGESIZE));

    if (xhci_reset(ctx) < 0) return -1;

    mmio_write32((volatile void *)(ctx->op_base + XHCI_CONFIG), ctx->max_slots);

    ctx->dcbaa = (uint64_t *)shim_dma_alloc(
        sizeof(uint64_t) * (ctx->max_slots + 1), &ctx->dcbaa_dma);
    if (!ctx->dcbaa) return -2;
    mmio_write64((volatile void *)(ctx->op_base + XHCI_DCBAAP), ctx->dcbaa_dma);

    ctx->num_scratchpad = (ctx->hcs_params2 >> 27) & 0x1F;
    if (ctx->num_scratchpad > MAX_SCRATCHPAD_BUF)
        ctx->num_scratchpad = MAX_SCRATCHPAD_BUF;
    for (uint32_t i = 0; i < ctx->num_scratchpad; i++) {
        ctx->scratchpad[i] = shim_dma_alloc(4096, &ctx->scratchpad_dma[i]);
        if (ctx->scratchpad[i]) ctx->dcbaa[i] = ctx->scratchpad_dma[i];
    }

    if (cmd_ring_init(ctx) < 0) return -3;
    if (evt_ring_init(ctx) < 0) return -4;
    if (xhci_start(ctx) < 0) return -5;

    ctx->hc_ready = true;
    return 0;
}
