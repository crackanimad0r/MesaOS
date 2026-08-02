/*
 * usb_shim_descriptors.c - Parseo y cache de descriptors USB
 *
 * Traduce Device Descriptor, String Descriptors y Configuration Descriptor
 * a informacion legible: VID/PID, nombres, clase, velocidad.
 *
 * License: MIT
 */

#include "usb_shim_descriptors.h"
#include "usb_shim_core.h"
#include <string.h>

static struct usb_device_info dev_cache[MAX_SLOTS];

const char *usb_speed_to_string(uint8_t speed) {
    switch (speed) {
        case SPEED_LOW:    return "Low-Speed (1.5 Mbps)";
        case SPEED_FULL:   return "Full-Speed (12 Mbps)";
        case SPEED_HIGH:   return "High-Speed (480 Mbps)";
        case SPEED_SUPER:  return "SuperSpeed (5 Gbps)";
        default:           return "Unknown";
    }
}

const char *usb_class_to_string(uint8_t class_code) {
    switch (class_code) {
        case 0x00: return "Per-Interface";
        case 0x01: return "Audio";
        case 0x02: return "Communications";
        case 0x03: return "HID (Human Interface)";
        case 0x05: return "Physical";
        case 0x06: return "Image";
        case 0x07: return "Printer";
        case 0x08: return "Mass Storage";
        case 0x09: return "Hub";
        case 0x0A: return "CDC-Data";
        case 0x0B: return "Chip/SmartCard";
        case 0x0D: return "Content Security";
        case 0x0E: return "Video";
        case 0x0F: return "Personal Healthcare";
        case 0x10: return "Audio/Video Devices";
        case 0x11: return "Billboard";
        case 0x12: return "USB-C Bridge";
        case 0xEF: return "Vendor-specific";
        case 0xFE: return "Wireless Controller";
        case 0xFF: return "Vendor-specific (class=FF)";
        default:   return "Unknown";
    }
}

static void decode_string_descriptor(const uint8_t *buf, uint16_t len,
                                     char *out, size_t out_sz)
{
    /* USB String Descriptor: bLength, bDescriptorType(0x03), then UTF-16LE */
    if (!buf || !out || out_sz == 0 || len < 4) {
        if (out && out_sz) out[0] = '\0';
        return;
    }
    size_t pos = 0;
    size_t max = out_sz - 1;
    for (size_t i = 2; i + 1 < len && pos < max; i += 2) {
        uint16_t wc = (uint16_t)buf[i] | ((uint16_t)buf[i + 1] << 8);
        if (wc < 0x20 || wc > 0x7E) wc = '?';
        out[pos++] = (char)wc;
    }
    out[pos] = '\0';
}

void usb_decode_device_descriptor(struct usb_shim_context *ctx,
                                  uint8_t slot_id,
                                  const uint8_t *buf, uint16_t len)
{
    if (!ctx || !buf || len < 18 || slot_id == 0 || slot_id > MAX_SLOTS)
        return;

    struct usb_device_info *info = &dev_cache[slot_id - 1];
    memset(info, 0, sizeof(*info));
    info->slot_id = slot_id;

    /* Device Descriptor layout (USB 2.0 spec Table 9.3) */
    info->vendor_id      = (uint16_t)buf[8]  | ((uint16_t)buf[9]  << 8);
    info->product_id     = (uint16_t)buf[10] | ((uint16_t)buf[11] << 8);
    info->device_class   = buf[4];
    info->device_subclass = buf[5];
    info->device_protocol = buf[6];
    info->config_value   = buf[17];

    /* Port y velocidad */
    int port_idx = slot_id - 1;
    if (port_idx < (int)ctx->num_devices) {
        info->port  = ctx->devices[port_idx].port;
        info->speed = ctx->devices[port_idx].speed;
    }

    /* Strings si el buffer trae mas datos (manufacturer/product/serial) */
    if (len > 18) {
        decode_string_descriptor(&buf[18], (uint16_t)(len - 18),
                                 info->manufacturer, sizeof(info->manufacturer));
    }

    info->configured = (info->config_value > 0);
}

struct usb_device_info *usb_get_device_info(struct usb_shim_context *ctx,
                                            uint8_t slot_id)
{
    if (!ctx || slot_id == 0 || slot_id > MAX_SLOTS)
        return NULL;
    struct usb_device_info *info = &dev_cache[slot_id - 1];
    if (!info->vendor_id && !info->manufacturer[0])
        return NULL;
    return info;
}
