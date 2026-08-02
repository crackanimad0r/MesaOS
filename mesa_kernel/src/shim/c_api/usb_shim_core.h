/*
 * usb_shim_core.h - Tipos xHCI internos del shim
 *
 * Solo define lo que NO está en usb_shim.h.
 * License: MIT
 */

#ifndef _MESA_USB_SHIM_CORE_H
#define _MESA_USB_SHIM_CORE_H

#include "usb_shim.h"
#include "usb_shim_types.h"
#include <stdbool.h>

#define TRB_ISP             (1 << 3)

/* Estructuras xHCI */
struct xhci_trb {
    uint64_t parameter;
    uint32_t status;
    uint32_t control;
} __attribute__((packed));

struct xhci_seg {
    struct xhci_trb trbs[256];
    struct xhci_seg *next;
    uint64_t dma_addr;
};

struct xhci_erst_entry {
    uint64_t seg_addr;
    uint32_t seg_size;
    uint32_t rsvd;
} __attribute__((packed));

struct xhci_slot_ctx {
    uint32_t dev_info, dev_info2, tt_info, dev_state;
    uint32_t rsvd[4];
} __attribute__((packed));

struct xhci_ep_ctx {
    uint32_t ep_info, ep_info2;
    uint64_t deq_ptr;
    uint32_t tx_info, rsvd[3];
} __attribute__((packed));

struct xhci_input_ctx {
    uint32_t drop_flags, add_flags;
    uint32_t rsvd[6];
    struct xhci_slot_ctx slot;
    struct xhci_ep_ctx eps[MAX_ENDPOINTS];
} __attribute__((packed));

struct xhci_dev_ctx {
    struct xhci_slot_ctx slot;
    struct xhci_ep_ctx eps[MAX_ENDPOINTS];
} __attribute__((packed));

/* Funciones internas (no en usb_shim.h) */
void *shim_dma_alloc(size_t size, uint64_t *phys_out);
int  xhci_init(struct usb_shim_context *, uint64_t mmio_phys, uint64_t mmio_size);
void evt_ring_process(struct usb_shim_context *);
int  cmd_ring_enqueue(struct usb_shim_context *, struct xhci_trb *);
void port_reset(struct usb_shim_context *, int port);
int  submit_control_urb(struct usb_shim_context *, struct shim_usb_dev *,
                        uint8_t, uint8_t, uint16_t, uint16_t,
                        void *, uint16_t, uint32_t);
int  submit_bulk_urb(struct usb_shim_context *, struct shim_usb_dev *,
                     uint8_t, void *, int, uint32_t);

/* MMIO helpers */
static inline uint32_t mmio_read32(volatile void *a) { return *(volatile uint32_t *)a; }
static inline void mmio_write32(volatile void *a, uint32_t v) { *(volatile uint32_t *)a = v; }
static inline uint64_t mmio_read64(volatile void *a) { return *(volatile uint64_t *)a; }
static inline void mmio_write64(volatile void *a, uint64_t v) { *(volatile uint64_t *)a = v; }

#endif
