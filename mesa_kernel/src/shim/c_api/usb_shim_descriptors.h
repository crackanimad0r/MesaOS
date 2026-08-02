/*
 * usb_shim_descriptors.h - Parsing y storage de USB descriptors
 *
 * Permite obtener VID/PID, nombres, velocidad y clase.
 * License: MIT
 */

#ifndef _MESA_USB_SHIM_DESCRIPTORS_H
#define _MESA_USB_SHIM_DESCRIPTORS_H

#include "usb_shim.h"

/*
 * Constantes USB (SPEED_* y xHCI registers en usb_shim_types.h via usb_shim_core.h)
 */

#define USB_DT_DEVICE        0x01
#define USB_DT_CONFIG        0x02
#define USB_DT_STRING        0x03
#define USB_DT_INTERFACE     0x04
#define USB_DT_ENDPOINT      0x05

#define USB_CLASS_PER_INTERFACE 0x00
#define USB_CLASS_HUB           0x09

/* ══════════════════════════════════════════════════════════════════════════
 * Cache de descriptor (parcial: DEVICE + STRING man page)
 * ══════════════════════════════════════════════════════════════════════════ */

struct usb_device_info {
    uint8_t  slot_id;
    uint8_t  port;
    uint8_t  speed;
    uint8_t  addr;                /* Asignado por xHCI */
    uint16_t vendor_id;
    uint16_t product_id;
    uint8_t  device_class;
    uint8_t  device_subclass;
    uint8_t  device_protocol;
    uint8_t  config_value;
    bool     configured;
    char     manufacturer[64];
    char     product[64];
    char     serial[64];
};

struct usb_device_info *usb_get_device_info(struct usb_shim_context *ctx,
                                            uint8_t slot_id);
const char *usb_speed_to_string(uint8_t speed);
const char *usb_class_to_string(uint8_t class_code);
void usb_decode_device_descriptor(struct usb_shim_context *ctx,
                                  uint8_t slot_id,
                                  const uint8_t *buf, uint16_t len);

#endif /* _MESA_USB_SHIM_DESCRIPTORS_H */
