/*
 * usb_shim_ports.c - Gestión de puertos y enumeración USB
 *
 * License: MIT
 */

#include "usb_shim_core.h"
#include <string.h>

static volatile uint32_t *port_reg(struct usb_shim_context *ctx, int port) {
    return (volatile uint32_t *)(ctx->op_base + XHCI_PORTSC_BASE + port * 0x10);
}

static void port_power_on(struct usb_shim_context *ctx, int port) {
    uint32_t v = mmio_read32(port_reg(ctx, port));
    mmio_write32(port_reg(ctx, port), v | PORTSC_PP);
}

void port_reset(struct usb_shim_context *ctx, int port) {
    uint32_t v = mmio_read32(port_reg(ctx, port));
    mmio_write32(port_reg(ctx, port), v | PORTSC_PR);
    for (int t = 10000; t; t--) {
        if (!(mmio_read32(port_reg(ctx, port)) & PORTSC_PR)) break;
        for (volatile int i = 0; i < 100; i++);
    }
}

static int port_get_speed(struct usb_shim_context *ctx, int port) {
    return (mmio_read32(port_reg(ctx, port)) >> PORTSC_SPEED_SHIFT) & 0xF;
}

static bool port_connected(struct usb_shim_context *ctx, int port) {
    return (mmio_read32(port_reg(ctx, port)) & PORTSC_CCS) != 0;
}


/* ══════════════════════════════════════════════════════════════════════════
 * Enumeración de dispositivo en puerto
 * ══════════════════════════════════════════════════════════════════════════ */

static int enumerate_port(struct usb_shim_context *ctx, int port) {
    if (!port_connected(ctx, port)) return 0;

    int speed = port_get_speed(ctx, port);
    int slot_id = ctx->num_devices + 1;
    if (slot_id >= MAX_SLOTS) return -1;

    struct shim_usb_dev *dev = &ctx->devices[slot_id - 1];
    memset(dev, 0, sizeof(*dev));
    dev->slot_id = slot_id;
    dev->speed   = speed;
    dev->port    = port;

    dev->dev_ctx = (struct xhci_dev_ctx *)shim_dma_alloc(
        sizeof(struct xhci_dev_ctx), &dev->dev_ctx_dma);
    if (!dev->dev_ctx) return -2;
    ctx->dcbaa[slot_id] = dev->dev_ctx_dma;

    dev->input_ctx = (struct xhci_input_ctx *)shim_dma_alloc(
        sizeof(struct xhci_input_ctx), &dev->input_ctx_dma);
    if (!dev->input_ctx) return -3;

    ctx->num_devices++;

    struct scm_event evt;
    evt.type       = EVT_URB_COMPLETE;
    evt.id         = (uint32_t)slot_id;
    evt.status     = SCM_OK;
    evt.actual_len = (uint32_t)speed;
    evt.data_ofs   = 0;
    evt.data_len   = 0;
    evt.reserved   = 0;
    scm_event_queue_push(&ctx->region->evt_queue, &evt);

    return slot_id;
}

/* ══════════════════════════════════════════════════════════════════════════
 * Poll de interrupciones (event ring + puertos)
 * ══════════════════════════════════════════════════════════════════════════ */

void shim_poll_irq(struct usb_shim_context *ctx) {
    if (!ctx || !ctx->hc_ready) return;

    evt_ring_process(ctx);

    uint32_t sts = mmio_read32((volatile void *)(ctx->op_base + XHCI_USBSTS));
    if (!(sts & XHCI_STS_PCD)) return;
    mmio_write32((volatile void *)(ctx->op_base + XHCI_USBSTS), sts);

    for (uint32_t p = 1; p <= ctx->max_ports; p++) {
        uint32_t ps = mmio_read32(port_reg(ctx, p));
        if (!(ps & PORTSC_CSC)) continue;

        mmio_write32(port_reg(ctx, p), ps | PORTSC_CSC);
        if (ps & PORTSC_CCS) {
            port_power_on(ctx, p);
            enumerate_port(ctx, p);
        } else {
            for (uint32_t i = 0; i < ctx->num_devices; i++) {
                if (ctx->devices[i].port == p) {
                    ctx->dcbaa[ctx->devices[i].slot_id] = 0;
                    memset(&ctx->devices[i], 0, sizeof(struct shim_usb_dev));
                    break;
                }
            }
        }
        if (ps & PORTSC_PLC)
            mmio_write32(port_reg(ctx, p), ps | PORTSC_PLC);
    }
}
