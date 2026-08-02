/*
 * usb_shim_api.c - API pública que invocan los drivers Linux
 *
 * Implementa: shim_usb_alloc_urb, shim_usb_submit_urb,
 *             shim_usb_control_msg, shim_usb_bulk_msg, etc.
 *
 * License: MIT
 */

#include "usb_shim_core.h"
#include <string.h>

/* Contexto global (definido en usb_shim_main.c) */
extern struct usb_shim_context shim_ctx;

/* ══════════════════════════════════════════════════════════════════════════
 * URB allocation / free
 * ══════════════════════════════════════════════════════════════════════════ */

struct urb *shim_usb_alloc_urb(int iso_packets, int mem_flags)
{
    (void)iso_packets; (void)mem_flags;
    struct urb *urb = (struct urb *)shim_dma_alloc(sizeof(struct urb), NULL);
    if (urb) memset(urb, 0, sizeof(*urb));
    return urb;
}

void shim_usb_free_urb(struct urb *urb)
{
    (void)urb;
}

/* ══════════════════════════════════════════════════════════════════════════
 * submit / kill URB
 * ══════════════════════════════════════════════════════════════════════════ */

int shim_usb_submit_urb(struct urb *urb, int mem_flags)
{
    (void)mem_flags;
    if (!urb) return -1;

    struct scm_command cmd;
    memset(&cmd, 0, sizeof(cmd));

    uint8_t dev_slot = 1;
    uint32_t pipe = urb->pipe;
    uint8_t endpoint = pipe & 0x0F;
    uint8_t ep_type  = (pipe >> 15) & 0x03;

    if (ep_type == 0) {
        cmd.type = SCM_USB_CONTROL;
        cmd.arg0 = dev_slot;
        if (urb->setup_packet) {
            cmd.arg0 |= ((uint64_t)urb->setup_packet[0] << 8);
            cmd.arg0 |= ((uint64_t)urb->setup_packet[1] << 16);
            cmd.arg1 = (uint64_t)(urb->setup_packet[2] | (urb->setup_packet[3] << 8)) |
                      ((uint64_t)(urb->setup_packet[4] | (urb->setup_packet[5] << 8)) << 16);
            cmd.arg2 = (uint64_t)(urb->setup_packet[6] | (urb->setup_packet[7] << 8));
        }
    } else {
        cmd.type = (ep_type == 2) ? SCM_USB_BULK : SCM_USB_CONTROL;
        cmd.arg0 = dev_slot | ((uint64_t)endpoint << 8);
        cmd.arg1 = urb->transfer_buffer_length;
    }

    if (urb->transfer_buffer && urb->transfer_buffer_length > 0) {
        int ofs = shim_data_pool_alloc(shim_ctx.region,
                                       urb->transfer_buffer_length);
        if (ofs >= 0) {
            memcpy(&shim_ctx.region->data_pool[ofs],
                   urb->transfer_buffer, urb->transfer_buffer_length);
            cmd.data_len = urb->transfer_buffer_length;
            cmd.data_ofs = (uint32_t)ofs;
        }
    }
    return scm_queue_push(&shim_ctx.region->cmd_queue, &cmd);
}

int shim_usb_kill_urb(struct urb *urb)
{
    (void)urb;
    return 0;
}

/* control / bulk messages síncronos */
int shim_usb_control_msg(struct usb_device *dev, unsigned int pipe,
                          uint8_t request, uint8_t requesttype,
                          uint16_t value, uint16_t index,
                          void *data, uint16_t size, int timeout)
{
    (void)dev; (void)timeout; (void)pipe;
    struct scm_command cmd = {0};
    cmd.type = SCM_USB_CONTROL;
    cmd.arg0 = 1 | ((uint64_t)requesttype << 8) | ((uint64_t)request << 16);
    cmd.arg1 = value | ((uint64_t)index << 16);
    cmd.arg2 = size;

    if (data && size > 0) {
        int ofs = shim_data_pool_alloc(shim_ctx.region, size);
        if (ofs >= 0) {
            if (!(requesttype & 0x80))
                memcpy(&shim_ctx.region->data_pool[ofs], data, size);
            cmd.data_len = size;
            cmd.data_ofs = (uint32_t)ofs;
        }
    }
    return (scm_queue_push(&shim_ctx.region->cmd_queue, &cmd) < 0) ? -1 : (int)size;
}

int shim_usb_bulk_msg(struct usb_device *dev, unsigned int pipe,
                      void *data, int len, int *actual, int timeout)
{
    (void)dev; (void)timeout;
    struct scm_command cmd = {0};
    cmd.type = SCM_USB_BULK;
    cmd.arg0 = 1 | (((uint64_t)(pipe & 0x0F)) << 8) |
              (((uint64_t)((pipe >> 8) & 0x01)) << 16);
    cmd.arg1 = (uint32_t)len;

    if (data && len > 0) {
        int ofs = shim_data_pool_alloc(shim_ctx.region, (size_t)len);
        if (ofs >= 0) {
            if (!((pipe >> 8) & 0x01))
                memcpy(&shim_ctx.region->data_pool[ofs], data, (size_t)len);
            cmd.data_len = (uint32_t)len;
            cmd.data_ofs = (uint32_t)ofs;
        }
    }
    int ret = scm_queue_push(&shim_ctx.region->cmd_queue, &cmd);
    if (ret < 0) return ret;
    if (actual) *actual = len;
    return len;
}

int shim_usb_reset_device(struct usb_device *dev)
{
    (void)dev;
    struct scm_command cmd = {0};
    cmd.type = SCM_USB_RESET_DEVICE;
    cmd.arg0 = 1;
    return scm_queue_push(&shim_ctx.region->cmd_queue, &cmd);
}

/* Descriptors / Interface */
int shim_usb_get_descriptor(struct usb_device *dev, uint8_t type,
                            uint8_t index, void *buf, int size)
{
    (void)dev;
    struct scm_command cmd = {0};
    cmd.type = SCM_USB_GET_DESCRIPTOR;
    cmd.arg0 = 1 | ((uint64_t)type << 8) | ((uint64_t)index << 16);
    cmd.data_len = (uint32_t)size;
    if (buf && size > 0) {
        int ofs = shim_data_pool_alloc(shim_ctx.region, (size_t)size);
        if (ofs >= 0) cmd.data_ofs = (uint32_t)ofs;
    }
    return scm_queue_push(&shim_ctx.region->cmd_queue, &cmd);
}

int shim_usb_set_interface(struct usb_device *dev, int interface, int alternate)
{
    (void)dev; (void)interface; (void)alternate;
    return 0;
}

int shim_usb_claim_interface(struct usb_device *dev, struct usb_interface *intf)
{
    (void)dev; (void)intf;
    return 0;
}

int shim_usb_release_interface(struct usb_device *dev, struct usb_interface *intf)
{
    (void)dev; (void)intf;
    return 0;
}

