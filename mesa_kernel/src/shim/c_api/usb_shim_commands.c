/*
 * usb_shim_commands.c - Manejador de comandos SCM del kernel
 *
 * Traduce cada comando SCM en operaciones reales xHCI.
 * License: MIT
 */

#include "usb_shim_core.h"
#include "usb_shim_descriptors.h"
#include <string.h>

/* Dispatch de comandos SCM */
void shim_handle_command(struct usb_shim_context *ctx,
                         const struct scm_command *cmd)
{
    struct scm_event evt = {0};
    evt.id = cmd->id;
    evt.status = SCM_OK;

    switch (cmd->type) {

    case SCM_NOP: evt.type = EVT_NONE; goto done;

    case SCM_USB_CONTROL: {
        uint8_t ds = cmd->arg0 & 0xFF, rt = (cmd->arg0 >> 8) & 0xFF;
        uint8_t rq = (cmd->arg0 >> 16) & 0xFF;
        uint16_t v = cmd->arg1 & 0xFFFF, ix = (cmd->arg1 >> 16) & 0xFFFF;
        uint16_t sz = cmd->arg2 & 0xFFFF;
        void *data = (cmd->data_len && cmd->data_ofs < SHIM_DATA_POOL_SIZE)
                     ? &ctx->region->data_pool[cmd->data_ofs] : NULL;
        if (ds > 0 && ds <= ctx->num_devices) {
            submit_control_urb(ctx, &ctx->devices[ds - 1],
                               rt, rq, v, ix, data, sz, cmd->id);
            evt.type = EVT_URB_COMPLETE; evt.actual_len = sz;
        } else { evt.type = EVT_URB_ERROR; evt.status = -ENODEV; }
        break;
    }

    case SCM_USB_BULK: {
        uint8_t ds = cmd->arg0 & 0xFF, ep = (cmd->arg0 >> 8) & 0xFF;
        int len = (int)(cmd->arg1 & 0xFFFFFFFF);
        void *data = (cmd->data_len && cmd->data_ofs < SHIM_DATA_POOL_SIZE)
                     ? &ctx->region->data_pool[cmd->data_ofs] : NULL;
        if (ds > 0 && ds <= ctx->num_devices) {
            submit_bulk_urb(ctx, &ctx->devices[ds - 1], ep, data, len, cmd->id);
            evt.type = EVT_URB_COMPLETE; evt.actual_len = (uint32_t)len;
        } else { evt.type = EVT_URB_ERROR; evt.status = -ENODEV; }
        break;
    }

    case SCM_USB_ALLOC_URB: case SCM_USB_FREE_URB:
    case SCM_USB_SUBMIT_URB: case SCM_USB_KILL_URB:
        evt.type = EVT_URB_COMPLETE; break;

    case SCM_USB_RESET_DEVICE: {
        uint8_t ds = cmd->arg0 & 0xFF;
        evt.type = EVT_URB_COMPLETE;
        if (ds > 0 && ds <= ctx->num_devices)
            port_reset(ctx, ctx->devices[ds - 1].port);
        else evt.status = -ENODEV;
        break;
    }

    case SCM_USB_GET_DESCRIPTOR: {
        uint8_t ds = cmd->arg0 & 0xFF, dt = (cmd->arg0 >> 8) & 0xFF;
        uint8_t di = (cmd->arg0 >> 16) & 0xFF;
        uint16_t li = cmd->arg1 & 0xFFFF, wl = cmd->data_len ? cmd->data_len : 256;
        void *buf = (cmd->data_ofs < SHIM_DATA_POOL_SIZE)
                    ? &ctx->region->data_pool[cmd->data_ofs] : NULL;
        if (ds > 0 && ds <= ctx->num_devices) {
            submit_control_urb(ctx, &ctx->devices[ds - 1],
                               0x80 | (dt == 3 ? 1 : 0), 6,
                               (uint16_t)((dt << 8) | di), li, buf, wl, cmd->id);
            evt.type = EVT_URB_COMPLETE; evt.actual_len = wl;
        } else { evt.type = EVT_URB_ERROR; evt.status = -ENODEV; }
        break;
    }

    case SCM_USB_SET_CONFIG: {
        uint8_t ds = cmd->arg0 & 0xFF, cf = (cmd->arg0 >> 8) & 0xFF;
        evt.type = EVT_URB_COMPLETE;
        if (ds > 0 && ds <= ctx->num_devices) {
            ctx->devices[ds - 1].config_value = cf;
            ctx->devices[ds - 1].configured = (cf > 0);
            submit_control_urb(ctx, &ctx->devices[ds - 1],
                               0, 9, cf, 0, NULL, 0, cmd->id);
        } else evt.status = -ENODEV;
        break;
    }

    case SCM_USB_CLAIM_INTF: case SCM_USB_RELEASE_INTF:
        evt.type = EVT_URB_COMPLETE; break;

    case SCM_SHIM_HEARTBEAT:
        evt.type = EVT_SHIM_HEARTBEAT_ACK;
        ctx->region->heartbeat_counter++;
        break;

    case SCM_USB_GET_DEVICE_INFO: {
        uint8_t ds = cmd->arg0 & 0xFF;
        if (ds > 0 && ds <= MAX_SLOTS) {
            struct usb_device_info *info = usb_get_device_info(ctx, ds);
            if (info && cmd->data_ofs < SHIM_DATA_POOL_SIZE) {
                uint32_t need = sizeof(struct usb_device_info);
                if (cmd->data_ofs + need <= SHIM_DATA_POOL_SIZE) {
                    memcpy(&ctx->region->data_pool[cmd->data_ofs],
                           info, need);
                    evt.type = EVT_URB_COMPLETE;
                    evt.actual_len = need;
                } else {
                    evt.type = EVT_URB_ERROR; evt.status = -ENOSPC;
                }
            } else {
                evt.type = EVT_URB_ERROR; evt.status = -ENODEV;
            }
        } else {
            evt.type = EVT_URB_ERROR; evt.status = -EINVAL;
        }
        break;
    }

    default:
        evt.type = EVT_URB_ERROR; evt.status = -EINVAL; break;
    }

done:
    if (evt.type != EVT_NONE)
        scm_event_queue_push(&ctx->region->evt_queue, &evt);
}

