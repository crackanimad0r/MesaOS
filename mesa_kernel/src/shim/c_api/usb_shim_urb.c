/*
 * usb_shim_urb.c - Envío de URBs Control/Bulk al controlador xHCI
 *
 * License: MIT
 */

#include "usb_shim_core.h"
#include <string.h>

/* ══════════════════════════════════════════════════════════════════════════
 * Control URB (Setup + Data + Status TRBs)
 * ══════════════════════════════════════════════════════════════════════════ */

int submit_control_urb(struct usb_shim_context *ctx,
                       struct shim_usb_dev *dev,
                       uint8_t rtype, uint8_t req,
                       uint16_t val, uint16_t idx,
                       void *data, uint16_t size,
                       uint32_t urb_id)
{
    (void)dev;
    uint32_t max_pkt = 64;

    /* Setup packet (8 bytes) */
    uint8_t setup[8] = { rtype, req,
                         val & 0xFF, val >> 8,
                         idx & 0xFF, idx >> 8,
                         size & 0xFF, size >> 8 };
    uint64_t setup_data;
    memcpy(&setup_data, setup, 8);

    struct xhci_trb trb;
    memset(&trb, 0, sizeof(trb));

    /* Setup Stage */
    trb.parameter = setup_data;
    trb.status    = (max_pkt << 16) | size;
    trb.control   = (TRB_SETUP << 10) | TRB_IOC | TRB_IDT |
                    ((rtype & 0x80) ? (3 << 16) : (2 << 16)) |
                    (urb_id & 0xFFFF);
    cmd_ring_enqueue(ctx, &trb);

    /* Data Stage (si hay) */
    if (size > 0 && data) {
        memset(&trb, 0, sizeof(trb));
        trb.parameter = (uint64_t)(uintptr_t)data;
        trb.status    = size;
        trb.control   = (TRB_DATA << 10) | TRB_IOC | TRB_CH |
                        ((rtype & 0x80) ? TRB_ISP : 0) |
                        (urb_id & 0xFFFF);
        cmd_ring_enqueue(ctx, &trb);
    }

    /* Status Stage */
    memset(&trb, 0, sizeof(trb));
    trb.control = (TRB_STATUS << 10) | TRB_IOC |
                  ((rtype & 0x80) ? (2 << 16) : (3 << 16)) |
                  (urb_id & 0xFFFF);
    cmd_ring_enqueue(ctx, &trb);
    return 0;
}

/* ══════════════════════════════════════════════════════════════════════════
 * Bulk/Interrupt URB (Normal TRB)
 * ══════════════════════════════════════════════════════════════════════════ */

int submit_bulk_urb(struct usb_shim_context *ctx,
                    struct shim_usb_dev *dev,
                    uint8_t endpoint, void *data, int len,
                    uint32_t urb_id)
{
    (void)dev;
    if (!data || len <= 0) return -1;

    struct xhci_trb trb;
    memset(&trb, 0, sizeof(trb));
    trb.parameter = (uint64_t)(uintptr_t)data;
    trb.status    = (uint32_t)len;
    trb.control   = (TRB_NORMAL << 10) | TRB_IOC | TRB_ISP |
                    (urb_id & 0xFFFF) |
                    ((endpoint & 0x0F) << 24);
    cmd_ring_enqueue(ctx, &trb);
    return 0;
}
