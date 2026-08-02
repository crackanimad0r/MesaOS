/*
 * usb_shim_main.c - Punto de entrada del shim USB
 *
 * Implementa shim_entry() y el bucle principal que:
 *   1. Inicializa xHCI
 *   2. Enciende puertos
 *   3. Bucle: poll IRQ → procesar comandos → esperar
 *
 * License: MIT
 */

#include "usb_shim_core.h"
#include <string.h>

/* Contexto global del shim */
struct usb_shim_context shim_ctx = {0};

/* Declaraciones externas (definidas en usb_shim_pool.c) */

/* ══════════════════════════════════════════════════════════════════════════
 * shim_entry - Punto de entrada (llamado por el loader del kernel)
 * ══════════════════════════════════════════════════════════════════════════ */

void shim_entry(uint64_t region_phys, uint64_t mmio_phys,
                uint64_t mmio_size, uint32_t pci_bdf)
{
    (void)pci_bdf;
    memset(&shim_ctx, 0, sizeof(shim_ctx));

    /* Mapear shared memory region */
    shim_ctx.region = (struct shim_region *)(uintptr_t)region_phys;

    /* Inicializar xHCI */
    int ret = xhci_init(&shim_ctx, mmio_phys, mmio_size);
    if (ret < 0) {
        struct scm_event evt = {
            .type = EVT_SHIM_ERROR,
            .status = ret,
            .id = (uint32_t)-ret
        };
        scm_event_queue_push(&shim_ctx.region->evt_queue, &evt);
        return;
    }

    /* Encender todos los puertos */
    for (uint32_t p = 1; p <= shim_ctx.max_ports; p++) {
        volatile uint32_t *portsc =
            (volatile uint32_t *)(shim_ctx.op_base + XHCI_PORTSC_BASE + p * 0x10);
        uint32_t v = mmio_read32(portsc);
        mmio_write32(portsc, v | PORTSC_PP);
    }

    shim_ctx.running = true;
    shim_ctx.region->flags |= SHIM_FLAG_RUNNING;

    /* Bucle principal: poll hardware + procesar comandos */
    while (shim_ctx.running) {
        shim_poll_irq(&shim_ctx);

        struct scm_command cmd;
        while (scm_queue_pop(&shim_ctx.region->cmd_queue, &cmd) == 0) {
            if (cmd.type == SCM_SHIM_PANIC) {
                shim_ctx.running = false;
                break;
            }
            shim_handle_command(&shim_ctx, &cmd);
        }

        /* Pequeña pausa para no saturar */
        for (volatile int i = 0; i < 1000; i++);
    }

    shim_ctx.region->flags &= ~SHIM_FLAG_RUNNING;
}
