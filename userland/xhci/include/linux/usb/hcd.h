#ifndef _LINUX_USB_HCD_H
#define _LINUX_USB_HCD_H
#include <linux/usb.h>
struct hc_driver {
    const char *description;
    const char *product_desc;
    size_t hcd_priv_size;
    int (*irq)(struct usb_hcd *hcd);
    int flags;
    int (*reset)(struct usb_hcd *hcd);
    int (*start)(struct usb_hcd *hcd);
    int (*pci_suspend)(struct usb_hcd *hcd, bool do_wakeup);
    int (*pci_resume)(struct usb_hcd *hcd, bool hibernated);
    void (*stop)(struct usb_hcd *hcd);
    void (*shutdown)(struct usb_hcd *hcd);
    int (*urb_enqueue)(struct usb_hcd *hcd, struct urb *urb, gfp_t mem_flags);
    int (*urb_dequeue)(struct usb_hcd *hcd, struct urb *urb, int status);
    int (*endpoint_disable)(struct usb_hcd *hcd, struct usb_host_endpoint *ep);
    int (*endpoint_reset)(struct usb_hcd *hcd, struct usb_host_endpoint *ep);
    int (*alloc_streams)(struct usb_hcd *hcd, struct usb_device *udev,
        struct usb_host_endpoint **eps, unsigned int num_eps,
        unsigned int num_streams, gfp_t mem_flags);
    int (*free_streams)(struct usb_hcd *hcd, struct usb_device *udev,
        struct usb_host_endpoint **eps, unsigned int num_eps,
        gfp_t mem_flags);
    void (*free_dev)(struct usb_hcd *hcd, struct usb_device *udev);
    int (*alloc_dev)(struct usb_hcd *hcd, struct usb_device *udev);
    void (*add_endpoint)(struct usb_hcd *hcd, struct usb_device *udev, struct usb_host_endpoint *ep);
    void (*drop_endpoint)(struct usb_hcd *hcd, struct usb_device *udev, struct usb_host_endpoint *ep);
    int (*check_bandwidth)(struct usb_hcd *hcd, struct usb_device *udev);
    void (*reset_bandwidth)(struct usb_hcd *hcd, struct usb_device *udev);
    int (*hub_control)(struct usb_hcd *hcd, u16 typeReq, u16 wValue, u16 wIndex, char *buf, u16 wLength);
    int (*hub_status_data)(struct usb_hcd *hcd, char *buf);
    int (*bus_suspend)(struct usb_hcd *hcd);
    int (*bus_resume)(struct usb_hcd *hcd);
    int (*get_resuming_ports)(struct usb_hcd *hcd);
    int (*update_hub_device)(struct usb_hcd *hcd, struct usb_device *hdev, struct usb_tt *tt, gfp_t mem_flags);
    void (*update_device)(struct usb_hcd *hcd, struct usb_device *udev);
    int (*reset_device)(struct usb_hcd *hcd, struct usb_device *udev);
    int (*get_frame_number)(struct usb_hcd *hcd);
    int (*set_usb2_hw_lpm)(struct usb_hcd *hcd, struct usb_device *udev, int enable);
    int (*enable_usb3_lpm_timeout)(struct usb_hcd *hcd, struct usb_device *udev, enum usb3_link_state state);
    int (*disable_usb3_lpm_timeout)(struct usb_hcd *hcd, struct usb_device *udev, enum usb3_link_state state);
    int (*find_raw_port_number)(struct usb_hcd *hcd, int port1);
    void (*clear_tt_buffer_complete)(struct usb_hcd *hcd, struct usb_host_endpoint *ep);
    void (*map_urb_for_dma)(struct usb_hcd *hcd, struct urb *urb, gfp_t mem_flags);
    void (*unmap_urb_for_dma)(struct usb_hcd *hcd, struct urb *urb);
    int (*address_device)(struct usb_hcd *hcd, struct usb_device *udev);
    int (*enable_device)(struct usb_hcd *hcd, struct usb_device *udev);
};
struct usb_hcd {
    void *hcd_priv;
};
#endif
