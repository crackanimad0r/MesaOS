/*
 * usb_shim_xhci.h - Declaraciones del controlador xHCI
 *
 * License: MIT
 */

#ifndef _MESA_USB_SHIM_XHCI_H
#define _MESA_USB_SHIM_XHCI_H

#include "usb_shim_core.h"

int  xhci_init(struct usb_shim_context *ctx, uint64_t mmio_phys, uint64_t mmio_size);
void evt_ring_process(struct usb_shim_context *ctx);
int  cmd_ring_enqueue(struct usb_shim_context *ctx, struct xhci_trb *trb);

#endif /* _MESA_USB_SHIM_XHCI_H */
