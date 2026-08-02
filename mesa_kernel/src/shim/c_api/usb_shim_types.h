/*
 * usb_shim_types.h - Tipos internos del shim xHCI (privado)
 *
 * License: MIT
 */

#ifndef _MESA_USB_SHIM_TYPES_H
#define _MESA_USB_SHIM_TYPES_H

#include "usb_shim.h"
#include <stdbool.h>

/* Registros xHCI */
#define XHCI_CAPLENGTH      0x00
#define XHCI_HCSPARAMS1     0x04
#define XHCI_HCSPARAMS2     0x08
#define XHCI_HCSPARAMS3     0x0C
#define XHCI_HCCPARAMS1     0x10
#define XHCI_DBOFF          0x14
#define XHCI_RTSOFF         0x18

#define XHCI_USBCMD         0x00
#define XHCI_USBSTS         0x04
#define XHCI_PAGESIZE       0x08
#define XHCI_CRCR           0x18
#define XHCI_DCBAAP         0x30
#define XHCI_CONFIG         0x38
#define XHCI_PORTSC_BASE    0x400

#define XHCI_CMD_RUN        (1 << 0)
#define XHCI_CMD_HCRST      (1 << 1)
#define XHCI_STS_HCH        (1 << 0)
#define XHCI_STS_PCD        (1 << 4)
#define XHCI_STS_CNR        (1 << 11)

#define PORTSC_CCS          (1 << 0)
#define PORTSC_PED          (1 << 1)
#define PORTSC_PR           (1 << 4)
#define PORTSC_PP           (1 << 9)
#define PORTSC_SPEED_SHIFT  10
#define PORTSC_CSC          (1 << 17)
#define PORTSC_PLC          (1 << 22)

#define SPEED_FULL  1
#define SPEED_LOW   2
#define SPEED_HIGH  3
#define SPEED_SUPER 4

/* Tipos de TRB */
#define TRB_NORMAL          1
#define TRB_SETUP           2
#define TRB_DATA            3
#define TRB_STATUS          4
#define TRB_LINK            6
#define TRB_ENABLE_SLOT     9
#define TRB_DISABLE_SLOT    10
#define TRB_ADDRESS_DEV     11
#define TRB_CONFIG_EP       12
#define TRB_RESET_DEV       17

/* Códigos de completación */
#define TRB_SUCCESS          1
#define TRB_SHORT_PACKET    13
#define TRB_STALL           24

/* Flags de control TRB */
#define TRB_C               (1 << 0)
#define TRB_TC              (1 << 1)
#define TRB_CH              (1 << 5)
#define TRB_IOC             (1 << 6)
#define TRB_IDT             (1 << 7)

/* Límites */
#define MAX_SLOTS           32
#define MAX_ENDPOINTS       32
#define MAX_SCRATCHPAD_BUF  8
#define DMA_POOL_SIZE       (512 * 1024)  /* 512KB */

#endif /* _MESA_USB_SHIM_TYPES_H */
