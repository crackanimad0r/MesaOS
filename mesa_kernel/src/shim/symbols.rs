use crate::linux::*;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use lazy_static::lazy_static;

pub struct KernelSymbol {
    pub name: &'static str,
    pub addr: usize,
}

macro_rules! ksym {
    ($name:expr, $func:path) => {
        KernelSymbol {
            name: $name,
            addr: $func as usize,
        }
    };
}

macro_rules! ksym_data {
    ($name:expr, $data:expr) => {
        KernelSymbol {
            name: $name,
            addr: &$data as *const _ as usize,
        }
    };
}

lazy_static! {
    pub static ref KERNEL_SYMBOLS: Vec<KernelSymbol> = {
        vec![
            // === Memory allocation ===
            ksym!("kmalloc", __shim_kmalloc),
            ksym!("kfree", __shim_kfree),
            ksym!("kzalloc", __shim_kzalloc),
            ksym!("krealloc", __shim_krealloc),
            ksym!("kcalloc", __shim_kcalloc),
            ksym!("vmalloc", __shim_vmalloc),
            ksym!("vfree", __shim_vfree),
            ksym!("__kmalloc", __shim_kmalloc),
            // === String/memory operations ===
            ksym!("memcpy", __shim_memcpy),
            ksym!("memset", __shim_memset),
            ksym!("memmove", __shim_memmove),
            ksym!("memcmp", __shim_memcmp),
            ksym!("strlen", __shim_strlen),
            ksym!("strcmp", __shim_strcmp),
            ksym!("strncmp", __shim_strncmp),
            ksym!("strcpy", __shim_strcpy),
            ksym!("strncpy", __shim_strncpy),
            ksym!("strcat", __shim_strcat),
            ksym!("snprintf", __shim_snprintf),
            ksym!("sprintf", __shim_sprintf),
            ksym!("printk", __shim_printk),
            ksym!("pr_info", __shim_printk),
            ksym!("pr_err", __shim_printk_err),
            ksym!("pr_warn", __shim_printk_warn),
            ksym!("pr_debug", __shim_printk),
            ksym!("dev_info", __shim_dev_info),
            ksym!("dev_err", __shim_dev_err),
            ksym!("dev_warn", __shim_dev_warn),
            ksym!("dev_dbg", __shim_dev_info),
            // === Spinlock ===
            ksym!("spin_lock", __shim_spin_lock),
            ksym!("spin_unlock", __shim_spin_unlock),
            ksym!("spin_lock_irqsave", __shim_spin_lock_irqsave),
            ksym!("spin_unlock_irqrestore", __shim_spin_unlock_irqrestore),
            ksym!("spin_lock_init", __shim_spin_lock_init),
            ksym!("spin_lock_bh", __shim_spin_lock),
            ksym!("spin_unlock_bh", __shim_spin_unlock),
            // === Mutex ===
            ksym!("mutex_init", __shim_mutex_init),
            ksym!("mutex_lock", __shim_mutex_lock),
            ksym!("mutex_unlock", __shim_mutex_unlock),
            ksym!("mutex_trylock", __shim_mutex_trylock),
            // === Completion ===
            ksym!("init_completion", __shim_init_completion),
            ksym!("wait_for_completion", __shim_wait_for_completion),
            ksym!("complete", __shim_complete),
            ksym!("reinit_completion", __shim_reinit_completion),
            // === Timer ===
            ksym!("init_timer", __shim_init_timer),
            ksym!("setup_timer", __shim_setup_timer),
            ksym!("mod_timer", __shim_mod_timer),
            ksym!("del_timer", __shim_del_timer),
            ksym!("timer_pending", __shim_timer_pending),
            ksym!("add_timer", __shim_mod_timer),
            // === Workqueue ===
            ksym!("INIT_WORK", __shim_init_work),
            ksym!("schedule_work", __shim_schedule_work),
            ksym!("flush_work", __shim_flush_work),
            ksym!("schedule_work_on", __shim_schedule_work),
            ksym!("flush_scheduled_work", __shim_flush_work),
            // === Wait queue ===
            ksym!("init_waitqueue_head", __shim_init_waitqueue_head),
            ksym!("add_wait_queue", __shim_add_wait_queue),
            ksym!("remove_wait_queue", __shim_remove_wait_queue),
            ksym!("wake_up", __shim_wake_up),
            ksym!("wake_up_interruptible", __shim_wake_up),
            ksym!("wait_event_interruptible", __shim_wait_event),
            // === DMA ===
            ksym!("dma_alloc_coherent", __shim_dma_alloc_coherent),
            ksym!("dma_free_coherent", __shim_dma_free_coherent),
            ksym!("dma_map_single", __shim_dma_map_single),
            ksym!("dma_unmap_single", __shim_dma_unmap_single),
            ksym!("dma_sync_single_for_device", __shim_dma_sync_single_for_device),
            ksym!("dma_sync_single_for_cpu", __shim_dma_sync_single_for_cpu),
            // === I/O Ports ===
            ksym!("inb", __shim_inb),
            ksym!("inw", __shim_inw),
            ksym!("inl", __shim_inl),
            ksym!("outb", __shim_outb),
            ksym!("outw", __shim_outw),
            ksym!("outl", __shim_outl),
            ksym!("ioread8", __shim_ioread8),
            ksym!("iowrite8", __shim_iowrite8),
            ksym!("ioread32", __shim_ioread32),
            ksym!("iowrite32", __shim_iowrite32),
            ksym!("ioread64", __shim_ioread64),
            ksym!("iowrite64", __shim_iowrite64),
            ksym!("ioport_map", __shim_ioport_map),
            ksym!("ioport_unmap", __shim_ioport_unmap),
            ksym!("ioremap", __shim_ioremap),
            ksym!("iounmap", __shim_iounmap),
            // === PCI ===
            ksym!("pci_read_config_byte", __shim_pci_read_config_byte),
            ksym!("pci_read_config_word", __shim_pci_read_config_word),
            ksym!("pci_read_config_dword", __shim_pci_read_config_dword),
            ksym!("pci_write_config_byte", __shim_pci_write_config_byte),
            ksym!("pci_write_config_word", __shim_pci_write_config_word),
            ksym!("pci_write_config_dword", __shim_pci_write_config_dword),
            ksym!("pci_enable_device", __shim_pci_enable_device),
            ksym!("pci_disable_device", __shim_pci_disable_device),
            ksym!("pci_set_master", __shim_pci_set_master),
            ksym!("pci_resource_start", __shim_pci_resource_start),
            ksym!("pci_resource_end", __shim_pci_resource_end),
            ksym!("pci_resource_len", __shim_pci_resource_len),
            ksym!("pci_request_regions", __shim_pci_request_regions),
            ksym!("pci_release_regions", __shim_pci_release_regions),
            // === USB ===
            ksym!("usb_alloc_urb", __shim_usb_alloc_urb),
            ksym!("usb_free_urb", __shim_usb_free_urb),
            ksym!("usb_submit_urb", __shim_usb_submit_urb),
            ksym!("usb_kill_urb", __shim_usb_kill_urb),
            ksym!("usb_control_msg", __shim_usb_control_msg),
            ksym!("usb_bulk_msg", __shim_usb_bulk_msg),
            ksym!("usb_reset_device", __shim_usb_reset_device),
            ksym!("usb_get_descriptor", __shim_usb_get_descriptor),
            ksym!("usb_set_interface", __shim_usb_set_interface),
            ksym!("usb_register_driver", __shim_usb_register_driver),
            ksym!("usb_deregister", __shim_usb_deregister),
            ksym!("usb_ifnum_to_if", __shim_usb_ifnum_to_if),
            ksym!("usb_rcvctrlpipe", __shim_usb_rcvctrlpipe),
            ksym!("usb_sndctrlpipe", __shim_usb_sndctrlpipe),
            ksym!("usb_rcvbulkpipe", __shim_usb_rcvbulkpipe),
            ksym!("usb_sndbulkpipe", __shim_usb_sndbulkpipe),
            ksym!("usb_rcvintpipe", __shim_usb_rcvintpipe),
            ksym!("usb_sndintpipe", __shim_usb_sndintpipe),
            ksym!("usb_maxpacket", __shim_usb_maxpacket),
            ksym!("usb_get_dev", __shim_usb_get_dev),
            ksym!("usb_put_dev", __shim_usb_put_dev),
            ksym!("interface_to_usbdev", __shim_interface_to_usbdev),
            ksym!("usb_set_intfdata", __shim_usb_set_intfdata),
            ksym!("usb_get_intfdata", __shim_usb_get_intfdata),
            // === Delays ===
            ksym!("msleep", __shim_msleep),
            ksym!("mdelay", __shim_mdelay),
            ksym!("udelay", __shim_udelay),
            ksym!("ssleep", __shim_ssleep),
            // === Miscellaneous ===
            ksym!("schedule", __shim_schedule),
            ksym!("wmb", __shim_wmb),
            ksym!("rmb", __shim_rmb),
            ksym!("mb", __shim_mb),
            ksym!("barrier", __shim_barrier),
            ksym!("get_cycles", __shim_get_cycles),
            // === Module info ===
            ksym!("__this_module", __shim_this_module),
            // === List operations ===
            ksym!("list_add", __shim_list_add),
            ksym!("list_del", __shim_list_del),
            ksym!("list_empty", __shim_list_empty),
            ksym!("list_for_each_entry", __shim_list_for_each),
            // === Atomic operations ===
            ksym!("atomic_set", __shim_atomic_set),
            ksym!("atomic_read", __shim_atomic_read),
            ksym!("atomic_add", __shim_atomic_add),
            ksym!("atomic_sub", __shim_atomic_sub),
            ksym!("atomic_inc", __shim_atomic_inc),
            ksym!("atomic_dec", __shim_atomic_dec),
            ksym!("atomic_add_return", __shim_atomic_add_return),
            ksym!("atomic_sub_return", __shim_atomic_sub_return),
            ksym!("atomic_inc_return", __shim_atomic_inc_return),
            ksym!("atomic_dec_return", __shim_atomic_dec_return),
            // === Net operations ===
            ksym!("dev_alloc_skb", __shim_dev_alloc_skb),
            ksym!("kfree_skb", __shim_kfree_skb),
            ksym!("skb_put", __shim_skb_put),
            ksym!("skb_push", __shim_skb_push),
            ksym!("skb_reserve", __shim_skb_reserve),
            ksym!("skb_copy_to_linear_data", __shim_skb_copy_to_linear_data),
            ksym!("skb_copy_from_linear_data", __shim_skb_copy_from_linear_data),
            ksym!("eth_type_trans", __shim_eth_type_trans),
            ksym!("netif_rx", __shim_netif_rx),
            ksym!("netif_receive_skb", __shim_netif_receive_skb),
            ksym!("netif_start_queue", __shim_netif_start_queue),
            ksym!("netif_wake_queue", __shim_netif_wake_queue),
            ksym!("netif_stop_queue", __shim_netif_stop_queue),
            ksym!("register_netdev", __shim_register_netdev),
            ksym!("unregister_netdev", __shim_unregister_netdev),
            ksym!("alloc_etherdev", __shim_alloc_etherdev),
            ksym!("free_netdev", __shim_free_netdev),
            ksym!("netif_device_attach", __shim_netif_device_attach),
            ksym!("netif_device_detach", __shim_netif_device_detach),
            // === Module common ===
            ksym!("module_layout", __shim_module_layout),
            ksym!("param_ops_int", __shim_param_ops_int),
            ksym!("param_ops_charp", __shim_param_ops_charp),
            ksym!("param_ops_bool", __shim_param_ops_bool),
            ksym!("param_ops_uint", __shim_param_ops_uint),
            ksym!("param_ops_long", __shim_param_ops_long),
            ksym!("param_ops_ulong", __shim_param_ops_ulong),
            ksym!("request_firmware", __shim_request_firmware),
            ksym!("release_firmware", __shim_release_firmware),
            ksym!("try_module_get", __shim_try_module_get),
            ksym!("module_put", __shim_module_put),
            // === PCI Driver Model ===
            ksym!("pci_register_driver", __shim_pci_register_driver),
            ksym!("pci_unregister_driver", __shim_pci_unregister_driver),
            // === IRQ ===
            ksym!("request_irq", __shim_request_irq),
            ksym!("free_irq", __shim_free_irq),
            ksym!("enable_irq", __shim_enable_irq),
            ksym!("disable_irq", __shim_disable_irq),
            ksym!("synchronize_irq", __shim_synchronize_irq),
            // === Tasklet ===
            ksym!("tasklet_init", __shim_tasklet_init),
            ksym!("tasklet_schedule", __shim_tasklet_schedule),
            ksym!("tasklet_kill", __shim_tasklet_kill),
            ksym!("tasklet_hi_schedule", __shim_tasklet_hi_schedule),
            // === Softirq ===
            ksym!("raise_softirq", __shim_raise_softirq),
            // === Networking ===
            ksym!("dev_queue_xmit", __shim_dev_queue_xmit),
            ksym!("netif_carrier_on", __shim_netif_carrier_on),
            ksym!("netif_carrier_off", __shim_netif_carrier_off),
            // === mac80211 ===
            ksym!("ieee80211_alloc_hw", __shim_ieee80211_alloc_hw),
            ksym!("ieee80211_register_hw", __shim_ieee80211_register_hw),
            ksym!("ieee80211_unregister_hw", __shim_ieee80211_unregister_hw),
            ksym!("ieee80211_free_hw", __shim_ieee80211_free_hw),
            ksym!("ieee80211_stop_queues", __shim_ieee80211_stop_queues),
            ksym!("ieee80211_wake_queues", __shim_ieee80211_wake_queues),
            ksym!("ieee80211_stop_queue", __shim_ieee80211_stop_queue),
            ksym!("ieee80211_wake_queue", __shim_ieee80211_wake_queue),
            ksym!("ieee80211_tx_status_irqsafe", __shim_ieee80211_tx_status_irqsafe),
            ksym!("ieee80211_rx_napi", __shim_ieee80211_rx_napi),
            ksym!("ieee80211_rx_irqsafe", __shim_ieee80211_rx_irqsafe),
            ksym!("ieee80211_find_sta", __shim_ieee80211_find_sta),
            ksym!("ieee80211_find_sta_by_ifaddr", __shim_ieee80211_find_sta_by_ifaddr),
            ksym!("ieee80211_iterate_stations_atomic", __shim_ieee80211_iterate_stations_atomic),
            ksym!("ieee80211_iterate_active_interfaces_atomic", __shim_ieee80211_iterate_active_interfaces_atomic),
            ksym!("ieee80211_beacon_get_tim", __shim_ieee80211_beacon_get_tim),
            ksym!("ieee80211_scan_completed", __shim_ieee80211_scan_completed),
            ksym!("ieee80211_connection_loss", __shim_ieee80211_connection_loss),
            ksym!("ieee80211_queue_work", __shim_ieee80211_queue_work),
            ksym!("ieee80211_queue_delayed_work", __shim_ieee80211_queue_delayed_work),
            ksym!("ieee80211_channel_to_frequency", __shim_ieee80211_channel_to_frequency),
            ksym!("ieee80211_free_txskb", __shim_ieee80211_free_txskb),
            ksym!("ieee80211_tx_dequeue", __shim_ieee80211_tx_dequeue),
            ksym!("ieee80211_tx_info_clear_status", __shim_ieee80211_tx_info_clear_status),
            ksym!("ieee80211_txq_get_depth", __shim_ieee80211_txq_get_depth),
            ksym!("ieee80211_start_tx_ba_session", __shim_ieee80211_start_tx_ba_session),
            ksym!("ieee80211_stop_tx_ba_cb_irqsafe", __shim_ieee80211_stop_tx_ba_cb_irqsafe),
            ksym!("ieee80211_purge_tx_queue", __shim_ieee80211_purge_tx_queue),
            ksym!("ieee80211_restart_hw", __shim_ieee80211_restart_hw),
            ksym!("ieee80211_request_smps", __shim_ieee80211_request_smps),
            ksym!("ieee80211_cqm_rssi_notify", __shim_ieee80211_cqm_rssi_notify),
            ksym!("ieee80211_report_wowlan_wakeup", __shim_ieee80211_report_wowlan_wakeup),
            ksym!("ieee80211_create_tpt_led_trigger", __shim_ieee80211_create_tpt_led_trigger),
            ksym!("ieee80211_pspoll_get", __shim_ieee80211_pspoll_get),
            ksym!("ieee80211_nullfunc_get", __shim_ieee80211_nullfunc_get),
            ksym!("ieee80211_proberesp_get", __shim_ieee80211_proberesp_get),
            ksym!("ieee80211_probereq_get", __shim_ieee80211_probereq_get),
            ksym!("ieee80211_vif_type_p2p", __shim_ieee80211_vif_type_p2p),
            // === NAPI ===
            ksym!("netif_napi_add", __shim_netif_napi_add),
            ksym!("netif_napi_del", __shim_netif_napi_del),
            ksym!("napi_enable", __shim_napi_enable),
            ksym!("napi_disable", __shim_napi_disable),
            ksym!("napi_schedule", __shim_napi_schedule),
            ksym!("napi_synchronize", __shim_napi_synchronize),
            ksym!("napi_complete_done", __shim_napi_complete_done),
            // === PCI helpers ===
            ksym!("pci_iomap", __shim_pci_iomap),
            ksym!("pci_iounmap", __shim_pci_iounmap),
            ksym!("pci_alloc_irq_vectors", __shim_pci_alloc_irq_vectors),
            ksym!("pci_free_irq_vectors", __shim_pci_free_irq_vectors),
            ksym!("pcie_capability_read_word", __shim_pcie_capability_read_word),
            ksym!("pcie_capability_set_word", __shim_pcie_capability_set_word),
            ksym!("pci_upstream_bridge", __shim_pci_upstream_bridge),
            ksym!("pci_set_power_state", __shim_pci_set_power_state),
            ksym!("pci_enable_wake", __shim_pci_enable_wake),
            // === devm IRQ ===
            ksym!("devm_request_threaded_irq", __shim_devm_request_threaded_irq),
            ksym!("devm_free_irq", __shim_devm_free_irq),
            // === netdev ===
            ksym!("alloc_netdev_dummy", __shim_alloc_netdev_dummy),
            // === skb helpers ===
            ksym!("skb_copy", __shim_skb_copy),
            ksym!("skb_pull", __shim_skb_pull),
            // === completion ===
            ksym!("complete_all", __shim_complete_all),
            // === firmware ===
            ksym!("request_firmware_nowait", __shim_request_firmware_nowait),
            // === skb queue helpers ===
            ksym!("alloc_skb", __shim_alloc_skb),
            ksym!("dev_kfree_skb_any", __shim_dev_kfree_skb_any),
            ksym!("skb_dequeue", __shim_skb_dequeue),
            ksym!("skb_put_data", __shim_skb_put_data),
            ksym!("skb_queue_purge", __shim_skb_queue_purge),
            ksym!("__skb_queue_tail", __shim___skb_queue_tail),
            ksym!("skb_queue_tail", __shim_skb_queue_tail),
            ksym!("__skb_unlink", __shim___skb_unlink),
            ksym!("skb_unlink", __shim_skb_unlink),
            // === workqueue ===
            ksym!("alloc_workqueue", __shim_alloc_workqueue),
            ksym!("destroy_workqueue", __shim_destroy_workqueue),
            // === timer ===
            ksym!("timer_delete_sync", __shim_timer_delete_sync),
            // === devm helpers ===
            ksym!("devm_kmemdup", __shim_devm_kmemdup),
            // === regulatory ===
            ksym!("regulatory_hint", __shim_regulatory_hint),
            // === get_random_mask_addr ===
            ksym!("get_random_mask_addr", __shim_get_random_mask_addr),
            // === ieee80211_emulate ===
            ksym!("ieee80211_emulate_add_chanctx", __shim_ieee80211_emulate_add_chanctx),
            ksym!("ieee80211_emulate_remove_chanctx", __shim_ieee80211_emulate_remove_chanctx),
            ksym!("ieee80211_emulate_change_chanctx", __shim_ieee80211_emulate_change_chanctx),
            ksym!("ieee80211_emulate_switch_vif_chanctx", __shim_ieee80211_emulate_switch_vif_chanctx),
            // === jiffies (global variable) ===
            ksym_data!("jiffies", __shim_jiffies),
            // === popcount ===
            ksym!("__popcountdi2", __popcountdi2),
            // === cfg80211 ===
            ksym!("cfg80211_calculate_bitrate", __shim_cfg80211_calculate_bitrate),
            ksym!("cfg80211_ssid_eq", __shim_cfg80211_ssid_eq),
            ksym!("cfg80211_get_ies_channel_number", __shim_cfg80211_get_ies_channel_number),
            // === wiphy ===
            ksym!("wiphy_to_ieee80211_hw", __shim_wiphy_to_ieee80211_hw),
            // === XHCI Missing Symbols ===
            ksym!("__fentry__", __shim___fentry__),
            ksym!("__x86_return_thunk", __shim___x86_return_thunk),
            ksym!("__ubsan_handle_out_of_bounds", __shim___ubsan_handle_out_of_bounds),
            ksym!("_dev_warn", __shim_dev_warn),
            ksym!("__dynamic_dev_dbg", __shim_dev_info),
            ksym!("usb_hcd_is_primary_hcd", __shim_usb_hcd_is_primary_hcd),
            ksym!("_dev_info", __shim_dev_info),
            ksym!("usb_hcd_poll_rh_status", __shim_usb_hcd_poll_rh_status),
            ksym!("usb_hcd_resume_root_hub", __shim_usb_hcd_resume_root_hub),
            ksym!("__dynamic_pr_debug", __shim_printk),
            ksym!("_raw_spin_lock_irqsave", __shim_spin_lock_irqsave),
            ksym!("_raw_spin_unlock_irqrestore", __shim_spin_unlock_irqrestore),
            ksym!("usb_hcd_unmap_urb_for_dma", __shim_usb_hcd_unmap_urb_for_dma),
            ksym!("dma_unmap_page_attrs", __shim_dma_unmap_single),
            ksym!("sg_pcopy_from_buffer", __shim_sg_pcopy_from_buffer),
            ksym!("__sw_hweight32", __shim___sw_hweight32),
            ksym!("usb_hcd_map_urb_for_dma", __shim_usb_hcd_map_urb_for_dma),
            ksym!("__kmalloc_node_noprof", __shim_kmalloc),
            ksym!("is_vmalloc_addr", __shim_is_vmalloc_addr),
            ksym!("dma_map_page_attrs", __shim_dma_map_single),
            ksym!("sg_pcopy_to_buffer", __shim_sg_pcopy_to_buffer),
            ksym!("dev_driver_string", __shim_dev_driver_string),
            ksym!("__warn_printk", __shim_printk_warn),
            ksym_data!("page_offset_base", __shim_page_offset_base),
            ksym_data!("vmemmap_base", __shim_vmemmap_base),
            ksym_data!("phys_base", __shim_phys_base),
            // === XHCI Missing Symbols - Batch 2 ===
            ksym!("delayed_work_timer_fn", __shim_delayed_work_timer_fn),
            ksym!("timer_init_key", __shim_timer_init_key),
            ksym!("__init_swait_queue_head", __shim_init_swait_queue_head),
            ksym!("dmi_get_system_info", __shim_dmi_get_system_info),
            ksym!("strstr", __shim_strstr),
            ksym_data!("cpu_number", __shim_cpu_number),
            ksym_data!("__cpu_online_mask", __shim_cpu_online_mask),
            ksym_data!("__preempt_count", __shim_preempt_count),
            ksym!("__SCT__preempt_schedule_notrace", __shim_preempt_schedule_notrace),
            ksym!("schedule_timeout_uninterruptible", __shim_schedule_timeout_uninterruptible),
            ksym!("__kmalloc_noprof", __shim_kmalloc),
            ksym!("usb_hcd_check_unlink_urb", __shim_usb_hcd_check_unlink_urb),
            ksym!("_dev_err", __shim_dev_err),
            ksym!("usb_hcd_unlink_urb_from_ep", __shim_usb_hcd_unlink_urb_from_ep),
            ksym!("usb_hcd_giveback_urb", __shim_usb_hcd_giveback_urb),
            ksym!("__const_udelay", __shim_udelay),
            ksym!("_raw_spin_lock_irq", __shim_spin_lock),
            ksym!("_raw_spin_unlock_irq", __shim_spin_unlock),
            ksym!("usleep_range_state", __shim_usleep_range_state),
            ksym!("usb_asmedia_modifyflowcontrol", __shim_usb_asmedia_modifyflowcontrol),
            ksym!("usb_disable_xhci_ports", __shim_usb_disable_xhci_ports),
            // === XHCI Missing Symbols - Batch 3 ===
            ksym!("__mutex_init", __shim_mutex_init),
            ksym!("__x86_indirect_thunk_r13", __shim___x86_indirect_thunk_rX),
            ksym!("__x86_indirect_thunk_rax", __shim___x86_indirect_thunk_rX),
            ksym!("__x86_indirect_thunk_rbx", __shim___x86_indirect_thunk_rX),
            ksym!("iommu_get_domain_for_dev", __shim_iommu_get_domain_for_dev),
            ksym!("dma_set_mask", __shim_dma_set_mask),
            ksym!("dma_set_coherent_mask", __shim_dma_set_coherent_mask),
            ksym!("usb_amd_dev_put", __shim_usb_amd_dev_put),
            ksym!("usb_root_hub_lost_power", __shim_usb_root_hub_lost_power),
            ksym_data!("__ref_stack_chk_guard", __shim_stack_chk_guard),
            ksym!("__stack_chk_fail", __shim_stack_chk_fail),
            ksym!("ktime_get", __shim_ktime_get),
            ksym!("__SCT__might_resched", __shim_might_resched),
            ksym!("__SCT__preempt_schedule", __shim_preempt_schedule_notrace),
            ksym!("dma_pool_free", __shim_dma_pool_free),
            ksym_data!("random_kmalloc_seed", __shim_random_kmalloc_seed),
            ksym_data!("kmalloc_caches", __shim_kmalloc_caches),
            ksym!("__kmalloc_cache_node_noprof", __shim_kmalloc),
            ksym!("__kmalloc_cache_noprof", __shim_kmalloc),
            ksym!("dma_pool_alloc", __shim_dma_pool_alloc),
            ksym!("dma_alloc_attrs", __shim_dma_alloc_coherent),
            ksym!("dma_free_attrs", __shim_dma_free_coherent),
            ksym!("radix_tree_lookup", __shim_radix_tree_lookup),
            ksym!("radix_tree_maybe_preload", __shim_radix_tree_maybe_preload),
            ksym!("radix_tree_delete", __shim_radix_tree_delete),
            ksym!("radix_tree_insert", __shim_radix_tree_insert),
            ksym!("cancel_delayed_work_sync", __shim_cancel_delayed_work_sync),
            ksym!("dma_pool_destroy", __shim_dma_pool_destroy),
            ksym!("dma_pool_create_node", __shim_dma_pool_create_node),
            ksym!("platform_device_unregister", __shim_platform_device_unregister),
            ksym!("platform_device_alloc", __shim_platform_device_alloc),
            ksym!("platform_device_add_resources", __shim_platform_device_add_resources),
            ksym!("platform_device_add", __shim_platform_device_add),
            ksym!("__devm_add_action", __shim_devm_add_action),
            ksym!("device_create_managed_software_node", __shim_device_create_managed_software_node),
            ksym!("platform_device_put", __shim_platform_device_put),
            ksym!("___ratelimit", __shim_ratelimit),
            ksym!("usb_hcd_link_urb_to_ep", __shim_usb_hcd_link_urb_to_ep),
            ksym!("__msecs_to_jiffies", __shim_msecs_to_jiffies),
            ksym_data!("system_percpu_wq", __shim_system_percpu_wq),
            ksym!("mod_delayed_work_on", __shim_mod_delayed_work_on),
            ksym!("usb_amd_quirk_pll_enable", __shim_usb_amd_quirk_pll_enable),
            // === Batch 4: XHCI final USB + trace symbols ===
            ksym!("usb_hub_clear_tt_buffer", __shim_usb_hub_clear_tt_buffer),
            ksym!("cancel_delayed_work", __shim_cancel_delayed_work),
            ksym!("usb_hc_died", __shim_usb_hc_died),
            ksym!("wait_for_completion_timeout", __shim_wait_for_completion_timeout),
            ksym!("usb_wakeup_notification", __shim_usb_wakeup_notification),
            ksym!("usb_hcd_start_port_resume", __shim_usb_hcd_start_port_resume),
            ksym!("_raw_spin_lock", __shim_spin_lock),
            ksym!("_raw_spin_unlock", __shim_spin_unlock),
            ksym!("usb_hcd_end_port_resume", __shim_usb_hcd_end_port_resume),
            ksym!("__fortify_panic", __shim_fortify_panic),
            ksym!("usb_amd_quirk_pll_disable", __shim_usb_amd_quirk_pll_disable),
            ksym!("usb_acpi_power_manageable", __shim_usb_acpi_power_manageable),
            ksym!("usb_acpi_set_power_state", __shim_usb_acpi_set_power_state),
            ksym_data!("pci_bus_type", __shim_pci_bus_type),
            ksym!("pm_runtime_allow", __shim_pm_runtime_allow),
            ksym!("pm_runtime_forbid", __shim_pm_runtime_forbid),
            ksym!("usb_amd_pt_check_port", __shim_usb_amd_pt_check_port),
            ksym!("vsnprintf", __shim_vsnprintf),
            ksym_data!("this_cpu_off", __shim_this_cpu_off),
            ksym!("perf_trace_buf_alloc", __shim_perf_trace_buf_alloc),
            ksym!("perf_trace_run_bpf_submit", __shim_perf_trace_run_bpf_submit),
            ksym!("trace_event_buffer_reserve", __shim_trace_event_buffer_reserve),
            ksym!("trace_event_buffer_commit", __shim_trace_event_buffer_commit),
            ksym!("__trace_trigger_soft_disabled", __shim_trace_trigger_soft_disabled),
            // === Batch 5: Trace output, sysfs, kstrtou*, bpf, PM, TTY, idr, kfifo ===
            ksym!("trace_raw_output_prep", __shim_trace_raw_output_prep),
            ksym!("trace_event_printf", __shim_trace_event_printf),
            ksym!("trace_handle_return", __shim_trace_handle_return),
            ksym!("trace_print_symbols_seq", __shim_trace_print_symbols_seq),
            ksym!("trace_seq_acquire", __shim_trace_seq_acquire),
            ksym!("bpf_trace_run1", __shim_bpf_trace_run1),
            ksym!("bpf_trace_run2", __shim_bpf_trace_run2),
            ksym!("bpf_trace_run3", __shim_bpf_trace_run3),
            ksym!("kstrtouint", __shim_kstrtouint),
            ksym!("sysfs_emit", __shim_sysfs_emit),
            ksym!("kstrtou8", __shim_kstrtou8),
            ksym!("kstrtou16", __shim_kstrtou16),
            ksym!("strcspn", __shim_strcspn),
            ksym!("utf8s_to_utf16s", __shim_utf8s_to_utf16s),
            ksym!("__pm_runtime_idle", __shim___pm_runtime_idle),
            ksym!("__pm_runtime_resume", __shim___pm_runtime_resume),
            ksym!("sysfs_streq", __shim_sysfs_streq),
            ksym!("sysfs_create_groups", __shim_sysfs_create_groups),
            ksym!("sysfs_remove_groups", __shim_sysfs_remove_groups),
            ksym!("__tasklet_schedule", __shim___tasklet_schedule),
            ksym!("tasklet_setup", __shim_tasklet_setup),
            ksym!("tty_port_close", __shim_tty_port_close),
            ksym!("tty_port_open", __shim_tty_port_open),
            ksym!("idr_find", __shim_idr_find),
            ksym!("tty_port_install", __shim_tty_port_install),
            ksym!("__tty_insert_flip_string_flags", __shim___tty_insert_flip_string_flags),
            ksym!("tty_flip_buffer_push", __shim_tty_flip_buffer_push),
            ksym!("_printk", __shim_printk),
            ksym!("__kfifo_out", __shim___kfifo_out),
            ksym!("tty_wakeup", __shim_tty_wakeup),
            ksym!("__kfifo_in", __shim___kfifo_in),
            ksym!("__tty_port_tty_hangup", __shim___tty_port_tty_hangup),
            ksym!("tty_unregister_device", __shim_tty_unregister_device),
            ksym!("tty_port_destroy", __shim_tty_port_destroy),
            ksym!("idr_remove", __shim_idr_remove),
            ksym!("__kfifo_free", __shim___kfifo_free),
            ksym!("tty_port_init", __shim_tty_port_init),
            ksym!("idr_alloc", __shim_idr_alloc),
            ksym!("__kfifo_alloc", __shim___kfifo_alloc),
            ksym!("tty_port_register_device", __shim_tty_port_register_device),
            ksym!("__tty_alloc_driver", __shim___tty_alloc_driver),
            // === Batch 6: TTY, debugfs, seq, uaccess ===
            ksym_data!("tty_std_termios", __shim_tty_std_termios),
            ksym!("tty_register_driver", __shim_tty_register_driver),
            ksym!("tty_driver_kref_put", __shim_tty_driver_kref_put),
            ksym!("idr_destroy", __shim_idr_destroy),
            ksym!("tty_unregister_driver", __shim_tty_unregister_driver),
            ksym!("debugfs_get_aux", __shim_debugfs_get_aux),
            ksym!("single_open", __shim_single_open),
            ksym!("seq_printf", __shim_seq_printf),
            ksym!("kstrtou16_from_user", __shim_kstrtou16_from_user),
            ksym!("__check_object_size", __shim___check_object_size),
            ksym!("_copy_from_user", __shim__copy_from_user),
            ksym!("debugfs_create_regset32", __shim_debugfs_create_regset32),
            ksym!("debugfs_create_dir", __shim_debugfs_create_dir),
            ksym!("debugfs_create_file_full", __shim_debugfs_create_file_full),
            ksym!("debugfs_remove", __shim_debugfs_remove),
            // === Batch 7: xhci-hcd final + xhci-pci ===
            ksym!("scnprintf", __shim_scnprintf),
            ksym!("__kvmalloc_node_noprof", __shim___kvmalloc_node_noprof),
            ksym!("dma_get_sgtable_attrs", __shim_dma_get_sgtable_attrs),
            ksym!("sg_free_table", __shim_sg_free_table),
            ksym!("sg_alloc_table_from_pages_segment", __shim_sg_alloc_table_from_pages_segment),
            ksym!("kvfree", __shim_kvfree),
            ksym!("__ubsan_handle_shift_out_of_bounds", __shim___ubsan_handle_shift_out_of_bounds),
            ksym!("__ubsan_handle_load_invalid_value", __shim___ubsan_handle_load_invalid_value),
            ksym!("usb_disabled", __shim_usb_disabled),
            ksym_data!("usb_debug_root", __shim_usb_debug_root),
            ksym!("seq_lseek", __shim_seq_lseek),
            ksym!("seq_read", __shim_seq_read),
            ksym!("single_release", __shim_single_release),
            ksym_data!("param_ops_ullong", __shim_param_ops_ullong),
            ksym!("validate_usercopy_range", __shim_validate_usercopy_range),
            ksym!("trace_event_reg", __shim_trace_event_reg),
            ksym!("trace_event_raw_init", __shim_trace_event_raw_init),
            ksym!("pci_dev_get", __shim_pci_dev_get),
            ksym!("firmware_request_nowarn", __shim_firmware_request_nowarn),
            ksym!("pci_dev_put", __shim_pci_dev_put),
            ksym!("__pci_register_driver", __shim___pci_register_driver),
            ksym!("usb_hcd_pci_shutdown", __shim_usb_hcd_pci_shutdown),
            ksym_data!("usb_hcd_pci_pm_ops", __shim_usb_hcd_pci_pm_ops),
            ksym!("usb_create_hcd", __shim_usb_create_hcd),
            ksym!("usb_add_hcd", __shim_usb_add_hcd),
            ksym!("usb_remove_hcd", __shim_usb_remove_hcd),
            ksym!("usb_put_hcd", __shim_usb_put_hcd),
            ksym!("xhci_pci_common_probe", __shim_xhci_pci_common_probe),
            ksym!("xhci_pci_remove", __shim_xhci_pci_remove),
        ]
    };
}

extern "C" fn __shim_kmalloc(size: usize, flags: u32) -> *mut u8 {
    unsafe { kmalloc(size, flags) }
}

extern "C" fn __shim_kfree(ptr: *mut u8) {
    unsafe { kfree(ptr) }
}

extern "C" fn __shim_kzalloc(size: usize, flags: u32) -> *mut u8 {
    unsafe { kzalloc(size, flags) }
}

extern "C" fn __shim_krealloc(ptr: *mut u8, new_size: usize, flags: u32) -> *mut u8 {
    unsafe { krealloc(ptr, new_size, flags) }
}

extern "C" fn __shim_kcalloc(n: usize, size: usize, flags: u32) -> *mut u8 {
    unsafe { kcalloc(n, size, flags) }
}

extern "C" fn __shim_vmalloc(size: usize) -> *mut u8 {
    unsafe { vmalloc(size) }
}

extern "C" fn __shim_vfree(ptr: *mut u8) {
    unsafe { vfree(ptr) }
}

extern "C" fn __shim_memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    unsafe {
        core::ptr::copy_nonoverlapping(src, dst, n);
        dst
    }
}

extern "C" fn __shim_memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    unsafe {
        core::ptr::write_bytes(s, c as u8, n);
        s
    }
}

extern "C" fn __shim_memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    unsafe {
        core::ptr::copy(src, dst, n);
        dst
    }
}

extern "C" fn __shim_memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        unsafe {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a != b {
                return (a as i32) - (b as i32);
            }
        }
    }
    0
}

extern "C" fn __shim_strlen(s: *const u8) -> usize {
    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

extern "C" fn __shim_strcmp(s1: *const u8, s2: *const u8) -> i32 {
    let mut i = 0;
    unsafe {
        loop {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a == 0 && b == 0 {
                return 0;
            }
            if a != b {
                return (a as i32) - (b as i32);
            }
            i += 1;
        }
    }
}

extern "C" fn __shim_strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        unsafe {
            let a = *s1.add(i);
            let b = *s2.add(i);
            if a == 0 && b == 0 {
                return 0;
            }
            if a != b {
                return (a as i32) - (b as i32);
            }
        }
    }
    0
}

extern "C" fn __shim_strcpy(dst: *mut u8, src: *const u8) -> *mut u8 {
    let mut i = 0;
    unsafe {
        loop {
            let c = *src.add(i);
            *dst.add(i) = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
    dst
}

extern "C" fn __shim_strncpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    unsafe {
        let len = core::ffi::CStr::from_ptr(src as *const i8).to_bytes().len();
        let copy_len = if len < n { len } else { n - 1 };
        core::ptr::copy_nonoverlapping(src, dst, copy_len);
        *dst.add(copy_len) = 0;
    }
    dst
}

extern "C" fn __shim_strcat(dst: *mut u8, src: *const u8) -> *mut u8 {
    unsafe {
        let dst_len = __shim_strlen(dst);
        __shim_strcpy(dst.add(dst_len), src);
    }
    dst
}

fn _fmt_putc(pos: &mut usize, buf: *mut u8, size: usize, c: u8) {
    if *pos < size {
        unsafe {
            *buf.add(*pos) = c;
        }
    }
    *pos += 1;
}

fn _fmt_u64(pos: &mut usize, buf: *mut u8, size: usize, v: u64) {
    if v == 0 {
        _fmt_putc(pos, buf, size, b'0');
        return;
    }
    let mut tmp = [0u8; 20];
    let mut i = 20;
    let mut n = v;
    while n > 0 {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    for j in i..20 {
        _fmt_putc(pos, buf, size, tmp[j]);
    }
}

fn _fmt_i64(pos: &mut usize, buf: *mut u8, size: usize, v: i64) {
    if v < 0 {
        _fmt_putc(pos, buf, size, b'-');
        _fmt_u64(pos, buf, size, (v as u64).wrapping_neg());
    } else {
        _fmt_u64(pos, buf, size, v as u64);
    }
}

fn _fmt_hex(pos: &mut usize, buf: *mut u8, size: usize, v: u64, upper: bool) {
    let hex = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    if v == 0 {
        _fmt_putc(pos, buf, size, b'0');
        return;
    }
    let mut tmp = [0u8; 16];
    let mut i = 16;
    let mut n = v;
    while n > 0 {
        i -= 1;
        tmp[i] = hex[(n & 0xF) as usize];
        n >>= 4;
    }
    for j in i..16 {
        _fmt_putc(pos, buf, size, tmp[j]);
    }
}

fn _fmt_str(pos: &mut usize, buf: *mut u8, size: usize, ptr: u64) {
    if ptr == 0 {
        return;
    }
    let s = unsafe { core::ffi::CStr::from_ptr(ptr as *const i8) };
    if let Ok(s) = s.to_str() {
        for &b in s.as_bytes() {
            _fmt_putc(pos, buf, size, b);
        }
    } else {
        _fmt_putc(pos, buf, size, b'?');
    }
}

fn _do_vsprintf(buf: *mut u8, size: usize, fmt: *const u8, args: &[u64]) -> i32 {
    if buf.is_null() || fmt.is_null() || size == 0 {
        if size > 0 && !buf.is_null() {
            unsafe {
                *buf = 0;
            }
        }
        return 0;
    }
    let fmt_s = match unsafe { core::ffi::CStr::from_ptr(fmt as *const i8) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            unsafe {
                *buf = 0;
            }
            return 0;
        }
    };
    let mut pos = 0usize;
    let mut ai = 0usize;
    let b = fmt_s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'%' {
            _fmt_putc(&mut pos, buf, size, b[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= b.len() {
            break;
        }
        if b[i] == b'%' {
            _fmt_putc(&mut pos, buf, size, b'%');
            i += 1;
            continue;
        }
        while i < b.len()
            && (b[i] == b'-' || b[i] == b'+' || b[i] == b' ' || b[i] == b'#' || b[i] == b'0')
        {
            i += 1;
        }
        if i < b.len() && b[i] == b'*' {
            ai += 1;
            i += 1;
        } else {
            while i < b.len() && b[i] >= b'0' && b[i] <= b'9' {
                i += 1;
            }
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            if i < b.len() && b[i] == b'*' {
                ai += 1;
                i += 1;
            } else {
                while i < b.len() && b[i] >= b'0' && b[i] <= b'9' {
                    i += 1;
                }
            }
        }
        let mut lng = false;
        if i < b.len() && b[i] == b'l' {
            lng = true;
            i += 1;
            if i < b.len() && b[i] == b'l' {
                i += 1;
            }
        } else if i < b.len() && b[i] == b'z' {
            lng = true;
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let sp = b[i];
        i += 1;
        let val = if ai < args.len() { args[ai] } else { 0 };
        ai += 1;
        match sp {
            b'd' | b'i' => _fmt_i64(
                &mut pos,
                buf,
                size,
                if lng { val as i64 } else { val as i32 as i64 },
            ),
            b'u' => _fmt_u64(
                &mut pos,
                buf,
                size,
                if lng { val } else { val as u32 as u64 },
            ),
            b'x' => _fmt_hex(&mut pos, buf, size, val, false),
            b'X' => _fmt_hex(&mut pos, buf, size, val, true),
            b'p' => {
                _fmt_putc(&mut pos, buf, size, b'0');
                _fmt_putc(&mut pos, buf, size, b'x');
                _fmt_hex(&mut pos, buf, size, val, false);
            }
            b's' => _fmt_str(&mut pos, buf, size, val),
            b'c' => _fmt_putc(&mut pos, buf, size, (val & 0xFF) as u8),
            _ => {
                _fmt_putc(&mut pos, buf, size, b'%');
                _fmt_putc(&mut pos, buf, size, sp);
            }
        }
    }
    if pos < size {
        unsafe {
            *buf.add(pos) = 0;
        }
    } else if size > 0 {
        unsafe {
            *buf.add(size - 1) = 0;
        }
    }
    pos as i32
}

fn _strip_kern_prefix(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 3 && b[0] == b'<' && b[2] == b'>' && b[1] >= b'0' && b[1] <= b'7' {
        &s[3..]
    } else {
        s
    }
}

fn _fmt_printk(fmt: *const u8, args: &[u64], prefix: &str) {
    if fmt.is_null() {
        return;
    }
    let mut tmp = [0u8; 512];
    let written = _do_vsprintf(tmp.as_mut_ptr(), tmp.len(), fmt, args);
    if written <= 0 {
        return;
    }
    let s = core::str::from_utf8(&tmp[..written as usize]).unwrap_or("");
    let s = _strip_kern_prefix(s);
    let s = s.trim_end_matches(|c| c == '\n' || c == '\r');
    if s.is_empty() {
        return;
    }
    crate::mesa_println!("{}{}", prefix, s);
}

fn _fmt_dev(dev: *const u8, fmt: *const u8, args: &[u64], severity: &str) {
    if fmt.is_null() {
        return;
    }
    let mut tmp = [0u8; 512];
    let written = _do_vsprintf(tmp.as_mut_ptr(), tmp.len(), fmt, args);
    if written <= 0 {
        return;
    }
    let s = core::str::from_utf8(&tmp[..written as usize]).unwrap_or("");
    let s = _strip_kern_prefix(s);
    let s = s.trim_end_matches(|c| c == '\n' || c == '\r');
    if s.is_empty() {
        return;
    }
    crate::mesa_println!("[DEV:{:p}] {} {}", dev as *const (), severity, s);
}

unsafe extern "C" fn __shim_snprintf(
    buf: *mut u8,
    size: usize,
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
) -> i32 {
    _do_vsprintf(buf, size, fmt, &[a1, a2, a3])
}

unsafe extern "C" fn __shim_sprintf(
    buf: *mut u8,
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
) -> i32 {
    _do_vsprintf(buf, usize::MAX, fmt, &[a1, a2, a3, a4])
}

unsafe extern "C" fn __shim_printk(
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
) -> i32 {
    if fmt.is_null() {
        return 0;
    }
    _fmt_printk(fmt, &[a1, a2, a3, a4, a5], "[KERN_MOD] ");
    0
}

unsafe extern "C" fn __shim_printk_err(
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
) -> i32 {
    _fmt_printk(fmt, &[a1, a2, a3, a4, a5], "[KERN_MOD] ERROR: ");
    0
}

unsafe extern "C" fn __shim_printk_warn(
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
) -> i32 {
    _fmt_printk(fmt, &[a1, a2, a3, a4, a5], "[KERN_MOD] WARN: ");
    0
}

unsafe extern "C" fn __shim_dev_info(
    dev: *const u8,
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
) -> i32 {
    _fmt_dev(dev, fmt, &[a1, a2, a3, a4], "");
    0
}

unsafe extern "C" fn __shim_dev_err(
    dev: *const u8,
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
) -> i32 {
    _fmt_dev(dev, fmt, &[a1, a2, a3, a4], "ERROR:");
    0
}

unsafe extern "C" fn __shim_dev_warn(
    dev: *const u8,
    fmt: *const u8,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
) -> i32 {
    _fmt_dev(dev, fmt, &[a1, a2, a3, a4], "WARN:");
    0
}

use core::sync::atomic::{AtomicI32, Ordering};

fn atomic_ld(ptr: *mut u8) -> &'static mut AtomicI32 {
    unsafe { &mut *(ptr as *mut AtomicI32) }
}

extern "C" fn __shim_spin_lock(lock: *mut u8) {
    while atomic_ld(lock).swap(1, Ordering::Acquire) != 0 {
        while atomic_ld(lock).load(Ordering::Relaxed) != 0 {
            core::hint::spin_loop();
        }
    }
}

extern "C" fn __shim_spin_unlock(lock: *mut u8) {
    atomic_ld(lock).store(0, Ordering::Release);
}

extern "C" fn __shim_spin_lock_irqsave(lock: *mut u8, flags: *mut u64) {
    unsafe {
        *flags = if x86_64::instructions::interrupts::are_enabled() {
            1
        } else {
            0
        };
    }
    x86_64::instructions::interrupts::disable();
    while atomic_ld(lock).swap(1, Ordering::Acquire) != 0 {
        while atomic_ld(lock).load(Ordering::Relaxed) != 0 {
            core::hint::spin_loop();
        }
    }
}

extern "C" fn __shim_spin_unlock_irqrestore(lock: *mut u8, flags: u64) {
    atomic_ld(lock).store(0, Ordering::Release);
    if flags != 0 {
        unsafe {
            x86_64::instructions::interrupts::enable();
        }
    }
}

extern "C" fn __shim_spin_lock_init(_lock: *mut u8) {
    atomic_ld(_lock).store(0, Ordering::Relaxed);
}

extern "C" fn __shim_mutex_init(m: *mut u8) {
    atomic_ld(m).store(0, Ordering::Relaxed);
}

extern "C" fn __shim_mutex_lock(m: *mut u8) {
    while atomic_ld(m).swap(1, Ordering::Acquire) != 0 {
        crate::scheduler::yield_now();
    }
}

extern "C" fn __shim_mutex_unlock(m: *mut u8) {
    atomic_ld(m).store(0, Ordering::Release);
}

extern "C" fn __shim_mutex_trylock(m: *mut u8) -> i32 {
    if atomic_ld(m).swap(1, Ordering::Acquire) == 0 {
        1
    } else {
        0
    }
}

extern "C" fn __shim_init_completion(c: *mut u8) {
    atomic_ld(c).store(0, Ordering::Relaxed);
}

extern "C" fn __shim_wait_for_completion(c: *mut u8) {
    crate::printk!("[SHIM] wait_for_completion entered (c={:p})", c);
    let start = crate::curr_arch::get_ticks();
    while atomic_ld(c).load(Ordering::Acquire) == 0 {
        let elapsed = crate::curr_arch::get_ticks().wrapping_sub(start);
        if elapsed > 180 && (elapsed % 180) == 0 {
            crate::printk!(
                "[SHIM] wait_for_completion: still waiting... ({} ticks)",
                elapsed
            );
        }
        crate::scheduler::yield_now();
    }
    crate::printk!("[SHIM] wait_for_completion returned");
}

extern "C" fn __shim_complete(c: *mut u8) {
    atomic_ld(c).store(1, Ordering::Release);
}

extern "C" fn __shim_reinit_completion(c: *mut u8) {
    atomic_ld(c).store(0, Ordering::Relaxed);
}

struct ShimTimer {
    func: usize,
    data: u64,
    expires: u64,
}

static SHIM_TIMERS: spin::Mutex<BTreeMap<u64, ShimTimer>> = spin::Mutex::new(BTreeMap::new());
static NEXT_TIMER_ID: AtomicI32 = AtomicI32::new(1);

extern "C" fn __shim_init_timer(t: *mut u8) {
    let timer = t as *mut crate::linux::timer::timer_list;
    unsafe {
        crate::linux::timer::init_timer(timer);
    }
}

extern "C" fn __shim_setup_timer(t: *mut u8, func: usize, data: u64) {
    unsafe {
        let ptr = t as *mut u64;
        *ptr = func as u64;
        *(ptr.add(1)) = data;
    }
}

extern "C" fn __shim_mod_timer(t: *mut u8, expires: u64) -> i32 {
    let id = NEXT_TIMER_ID.fetch_add(1, Ordering::Relaxed);
    unsafe {
        *(t as *mut i32) = id;
    }
    let func = unsafe { *(t as *const u64) };
    let data = unsafe { *((t as *const u64).add(1)) };
    SHIM_TIMERS.lock().insert(
        id as u64,
        ShimTimer {
            func: func as usize,
            data,
            expires,
        },
    );
    0
}

extern "C" fn __shim_del_timer(t: *mut u8) -> i32 {
    let id = unsafe { *(t as *const i32) };
    if id != 0 {
        SHIM_TIMERS.lock().remove(&(id as u64));
        1
    } else {
        0
    }
}

extern "C" fn __shim_timer_pending(t: *const u8) -> i32 {
    let id = unsafe { *(t as *const i32) };
    if id != 0 && SHIM_TIMERS.lock().contains_key(&(id as u64)) {
        1
    } else {
        0
    }
}

struct ShimWork {
    func: usize,
}

static SHIM_WORKQUEUE: spin::Mutex<Vec<ShimWork>> = spin::Mutex::new(Vec::new());

extern "C" fn __shim_init_work(w: *mut u8) {
    unsafe {
        *(w as *mut usize) = 0;
    }
}

extern "C" fn __shim_schedule_work(w: *mut u8) -> i32 {
    let func = unsafe { *(w as *const usize) };
    if func != 0 {
        SHIM_WORKQUEUE.lock().push(ShimWork { func });
    }
    0
}

pub fn process_workqueue() {
    let works = core::mem::take(&mut *SHIM_WORKQUEUE.lock());
    for w in works {
        let f: extern "C" fn() = unsafe { core::mem::transmute(w.func) };
        f();
    }
}

extern "C" fn __shim_flush_work(_w: *mut u8) {
    process_workqueue();
}

extern "C" fn __shim_init_waitqueue_head(wq: *mut u8) {
    let wqh = wq as *mut crate::linux::wait::wait_queue_head;
    unsafe {
        (*wqh).head.init();
    }
}

extern "C" fn __shim_add_wait_queue(wq: *mut u8, entry: *mut u8) {
    let wqh = wq as *mut crate::linux::wait::wait_queue_head;
    let wqe = entry as *mut crate::linux::wait::wait_queue_entry;
    unsafe {
        (*wqh).head.add(&mut (*wqe).entry);
    }
}

extern "C" fn __shim_remove_wait_queue(wq: *mut u8, entry: *mut u8) {
    let wq = wq as *mut crate::linux::wait::wait_queue_head;
    let wqe = entry as *mut crate::linux::wait::wait_queue_entry;
    unsafe {
        (*wqe).entry.del();
    }
}

extern "C" fn __shim_wake_up(wq: *mut u8) -> i32 {
    let wq = wq as *mut crate::linux::wait::wait_queue_head;
    unsafe { crate::linux::wait::wake_up(&mut *wq) }
}

extern "C" fn __shim_wait_event(wq: *mut u8, condition: i32) -> i32 {
    if condition == 0 {
        crate::scheduler::yield_now();
    }
    0
}

extern "C" fn __shim_dma_alloc_coherent(
    _dev: *mut u8,
    size: usize,
    dma_addr: *mut u64,
    _gfp: u32,
) -> *mut u8 {
    unsafe {
        let (ptr, phys) = dma_alloc_coherent(size);
        if !dma_addr.is_null() {
            *dma_addr = phys;
        }
        ptr
    }
}

extern "C" fn __shim_dma_free_coherent(
    _dev: *mut u8,
    size: usize,
    _cpu_addr: *mut u8,
    dma_addr: u64,
) {
    unsafe {
        dma_free_coherent(dma_addr, size);
    }
}

extern "C" fn __shim_dma_map_single(_dev: *mut u8, cpu_addr: u64, size: usize, dir: i32) -> u64 {
    unsafe { dma_map_single(cpu_addr, dir) }
}

extern "C" fn __shim_dma_unmap_single(_dev: *mut u8, dma_addr: u64, _size: usize, dir: i32) {
    unsafe { dma_unmap_single(dma_addr, dir) }
}

extern "C" fn __shim_dma_sync_single_for_device(
    _dev: *mut u8,
    dma_addr: u64,
    _size: usize,
    _dir: i32,
) {
    unsafe { dma_sync_single_for_device(dma_addr) }
}

extern "C" fn __shim_dma_sync_single_for_cpu(
    _dev: *mut u8,
    dma_addr: u64,
    _size: usize,
    _dir: i32,
) {
    unsafe { dma_sync_single_for_cpu(dma_addr) }
}

extern "C" fn __shim_inb(port: u16) -> u8 {
    unsafe { inb(port) }
}

extern "C" fn __shim_inw(port: u16) -> u16 {
    unsafe { inw(port) }
}

extern "C" fn __shim_inl(port: u16) -> u32 {
    unsafe { inl(port) }
}

extern "C" fn __shim_outb(port: u16, val: u8) {
    unsafe {
        outb(port, val);
    }
}

extern "C" fn __shim_outw(port: u16, val: u16) {
    unsafe {
        outw(port, val);
    }
}

extern "C" fn __shim_outl(port: u16, val: u32) {
    unsafe {
        outl(port, val);
    }
}

extern "C" fn __shim_ioread8(addr: *mut u8) -> u8 {
    unsafe { ioread8(addr) }
}

extern "C" fn __shim_iowrite8(addr: *mut u8, val: u8) {
    unsafe {
        iowrite8(addr, val);
    }
}

extern "C" fn __shim_ioread32(addr: *mut u32) -> u32 {
    unsafe { ioread32(addr) }
}

extern "C" fn __shim_iowrite32(addr: *mut u32, val: u32) {
    unsafe {
        iowrite32(addr, val);
    }
}

extern "C" fn __shim_ioread64(addr: *mut u64) -> u64 {
    unsafe { ioread64(addr) }
}

extern "C" fn __shim_iowrite64(addr: *mut u64, val: u64) {
    unsafe {
        iowrite64(addr, val);
    }
}

extern "C" fn __shim_ioport_map(port: u64, _count: u32) -> *mut u8 {
    port as *mut u8
}

extern "C" fn __shim_ioport_unmap(_addr: *mut u8) {}

extern "C" fn __shim_ioremap(phys: u64, size: u64) -> *mut u8 {
    unsafe {
        match crate::memory::vmm::map_mmio(phys, size) {
            Ok(virt) => virt as *mut u8,
            Err(_) => core::ptr::null_mut(),
        }
    }
}

extern "C" fn __shim_iounmap(_addr: *mut u8) {}

fn pci_dev_to_bdf(dev: *mut u8) -> (u8, u8, u8) {
    unsafe {
        let ptr = dev as *const u32;
        let bdf = *ptr;
        let bus = ((bdf >> 8) & 0xFF) as u8;
        let device = ((bdf >> 3) & 0x1F) as u8;
        let function = (bdf & 0x07) as u8;
        (bus, device, function)
    }
}

extern "C" fn __shim_pci_read_config_byte(dev: *mut u8, offset: i32, val: *mut u8) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    let dword = crate::pci::pci_config_read(bus, device, function, offset as u8);
    let shift = ((offset as u8) & 0x03) * 8;
    let mut byte = ((dword >> shift) & 0xFF) as u8;
    // HACK: The Renesas xHCI driver checks config byte at 0xf4 to determine
    // whether its firmware is already running.  For non-Renesas controllers
    // (e.g. QEMU xHCI) this register returns 0, which the driver interprets
    // as "firmware IS running" -> takes a path that tries to download
    // firmware -> fails -> probe returns -2 without calling
    // xhci_pci_common_probe.  Returning 0x10 here makes the driver's check
    // return 0 ("firmware NOT running"), which falls through to the common
    // probe path that works for any xHCI controller.
    if offset == 0xf4 && byte == 0 {
        byte = 0x10;
    }
    unsafe {
        *val = byte;
    }
    0
}

extern "C" fn __shim_pci_read_config_word(dev: *mut u8, offset: i32, val: *mut u16) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    let dword = crate::pci::pci_config_read(bus, device, function, offset as u8);
    let shift = ((offset as u8) & 0x02) * 8;
    unsafe {
        *val = ((dword >> shift) & 0xFFFF) as u16;
    }
    0
}

extern "C" fn __shim_pci_read_config_dword(dev: *mut u8, offset: i32, val: *mut u32) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    unsafe {
        *val = crate::pci::pci_config_read(bus, device, function, offset as u8);
    }
    0
}

extern "C" fn __shim_pci_write_config_byte(dev: *mut u8, offset: i32, val: u8) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    let mut dword = crate::pci::pci_config_read(bus, device, function, offset as u8);
    let shift = ((offset as u8) & 0x03) * 8;
    dword = (dword & !(0xFF << shift)) | ((val as u32) << shift);
    crate::pci::pci_config_write(bus, device, function, offset as u8, dword);
    0
}

extern "C" fn __shim_pci_write_config_word(dev: *mut u8, offset: i32, val: u16) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    let mut dword = crate::pci::pci_config_read(bus, device, function, offset as u8);
    let shift = ((offset as u8) & 0x02) * 8;
    dword = (dword & !(0xFFFF << shift)) | ((val as u32) << shift);
    crate::pci::pci_config_write(bus, device, function, offset as u8, dword);
    0
}

extern "C" fn __shim_pci_write_config_dword(dev: *mut u8, offset: i32, val: u32) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    crate::pci::pci_config_write(bus, device, function, offset as u8, val);
    0
}

extern "C" fn __shim_pci_enable_device(dev: *mut u8) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    let mut cmd = crate::pci::pci_config_read(bus, device, function, 0x04);
    cmd |= 0x07; // IO + MEM + BusMaster
    crate::pci::pci_config_write(bus, device, function, 0x04, cmd);

    // Wake device from D3 (most laptops leave WiFi in D3 by default)
    // Find PM capability
    let caps = crate::pci::pci_config_read(bus, device, function, 0x34);
    let mut cap_ptr = (caps & 0xFF) as u8;
    while cap_ptr != 0 {
        let cap_dw = crate::pci::pci_config_read(bus, device, function, cap_ptr);
        let cap_id = (cap_dw & 0xFF) as u8;
        if cap_id == 0x01 {
            // PM capability found - write D0 to PMCSR (cap_ptr + 4)
            let pmcsr = crate::pci::pci_config_read(bus, device, function, cap_ptr + 4);
            if (pmcsr & 3) != 0 {
                crate::pci::pci_config_write(bus, device, function, cap_ptr + 4, pmcsr & !3);
                // Small delay (~1ms) for device to wake
                for _ in 0..1000000 {
                    core::hint::spin_loop();
                }
            }
            crate::mesa_println!(
                "[PCI] Device {:02x}:{:02x}.{:x} PM state -> D0",
                bus,
                device,
                function
            );
            break;
        }
        let next_raw = crate::pci::pci_config_read(bus, device, function, cap_ptr);
        cap_ptr = ((next_raw >> 8) & 0xFF) as u8;
    }

    0
}

extern "C" fn __shim_pci_disable_device(dev: *mut u8) {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    let mut cmd = crate::pci::pci_config_read(bus, device, function, 0x04);
    cmd &= !0x07;
    crate::pci::pci_config_write(bus, device, function, 0x04, cmd);
}

extern "C" fn __shim_pci_set_master(dev: *mut u8) {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    let mut cmd = crate::pci::pci_config_read(bus, device, function, 0x04);
    cmd |= 0x04; // Bus Master
    crate::pci::pci_config_write(bus, device, function, 0x04, cmd);
}

fn pci_dev_bar_info(dev: *mut u8, bar: i32) -> (u64, u64) {
    let (bus, device, function) = pci_dev_to_bdf(dev);
    match crate::pci::pci_read_bar(bus, device, function, bar as u8) {
        Some((start, size)) => (start, size),
        None => (0, 0),
    }
}

extern "C" fn __shim_pci_resource_start(dev: *mut u8, bar: i32) -> u64 {
    pci_dev_bar_info(dev, bar).0
}

extern "C" fn __shim_pci_resource_end(dev: *mut u8, bar: i32) -> u64 {
    let (start, size) = pci_dev_bar_info(dev, bar);
    if size > 0 {
        start + size - 1
    } else {
        0
    }
}

extern "C" fn __shim_pci_resource_len(dev: *mut u8, bar: i32) -> u64 {
    pci_dev_bar_info(dev, bar).1
}

static PCI_REQUESTED_REGIONS: spin::Mutex<alloc::collections::BTreeSet<u32>> =
    spin::Mutex::new(alloc::collections::BTreeSet::new());

extern "C" fn __shim_pci_request_regions(dev: *mut u8, _name: *const u8) -> i32 {
    if dev.is_null() {
        return -1;
    }
    let bdf = unsafe { *(dev as *const u32) };
    let mut regions = PCI_REQUESTED_REGIONS.lock();
    if regions.contains(&bdf) {
        return -16;
    } // -EBUSY
    regions.insert(bdf);
    0
}

extern "C" fn __shim_pci_release_regions(dev: *mut u8) {
    if dev.is_null() {
        return;
    }
    let bdf = unsafe { *(dev as *const u32) };
    PCI_REQUESTED_REGIONS.lock().remove(&bdf);
}

// ── PCI Driver Model ──────────────────────────────────────────
#[repr(C)]
#[repr(C)]
struct PciDeviceId {
    vendor: u32,
    device: u32,
    subvendor: u32,
    subdevice: u32,
    class: u32,
    class_mask: u32,
    driver_data: u64,
}

#[repr(C)]
#[derive(Clone)]
struct PciDriver {
    addr: u64,
    name: [u8; 64],
    id_table: u64,
    probe: u64,
    remove: u64,
}

static PCI_DRIVERS: spin::Mutex<Vec<PciDriver>> = spin::Mutex::new(Vec::new());

/// Allocate a fake `struct pci_dev` large enough for the module's probe function.
/// The BDF encoding is stored at offset 0 (read by `pci_dev_to_bdf`).
/// Returns null on allocation failure.
unsafe fn alloc_fake_pci_dev(bus: u8, device: u8, function: u8) -> *mut u8 {
    const PCI_DEV_SZ: usize = 4096;
    let dev = crate::linux::slab::kzalloc(PCI_DEV_SZ, 0);
    if dev.is_null() {
        return dev;
    }
    let bdf: u32 = (bus as u32) << 8 | (device as u32) << 3 | function as u32;
    *(dev as *mut u32) = bdf;
    crate::mesa_println!(
        "[PCI] Fake pci_dev at {:p} for {:02x}:{:02x}.{:x}",
        dev,
        bus,
        device,
        function
    );
    dev
}

unsafe fn pci_driver_call_probe(drv: &PciDriver, pci_dev: *mut u8, id: *const PciDeviceId) -> i32 {
    let (bus, device, function) = pci_dev_to_bdf(pci_dev);
    crate::mesa_println!(
        "[PCI] call_probe: probe={:#x} dev={:02x}:{:02x}.{:x}",
        drv.probe,
        bus,
        device,
        function
    );
    if drv.probe == 0 {
        crate::mesa_println!("[PCI] call_probe: probe is ZERO, skipping");
        return 0;
    }
    let func: extern "C" fn(*mut u8, *const PciDeviceId) -> i32 = core::mem::transmute(drv.probe);
    let ret = func(pci_dev, id);
    crate::mesa_println!("[PCI] probe returned: {}", ret);
    ret
}

unsafe fn pci_driver_call_remove(drv: &PciDriver, pci_dev: *mut u8) {
    if drv.remove == 0 {
        return;
    }
    let func: extern "C" fn(*mut u8) = core::mem::transmute(drv.remove);
    func(pci_dev);
}

unsafe fn pci_driver_match_and_probe(drv: &PciDriver) {
    let id_table = drv.id_table as *const PciDeviceId;
    if id_table.is_null() {
        return;
    }

    // Print ID table entries
    crate::mesa_println!("[PCI] ID table entries:");
    let mut i = 0;
    loop {
        let entry = id_table.add(i);
        if (*entry).vendor == 0
            && (*entry).device == 0
            && (*entry).subvendor == 0
            && (*entry).subdevice == 0
        {
            crate::mesa_println!("[PCI]   (end of table)");
            break;
        }
        crate::mesa_println!(
            "[PCI]   [{:02}] vendor={:#06x} device={:#06x} subv={:#06x} subd={:#06x} class={:#04x}",
            i,
            (*entry).vendor,
            (*entry).device,
            (*entry).subvendor,
            (*entry).subdevice,
            (*entry).class
        );
        i += 1;
    }

    // Print all PCI devices
    let devices = crate::pci::devices();
    crate::mesa_println!("[PCI] {} devices detected:", devices.len());
    for pci_dev in &devices {
        crate::mesa_println!(
            "[PCI]   {:02x}:{:02x}.{:x} {:04x}:{:04x} class={:02x}{:02x} ({})",
            pci_dev.bus,
            pci_dev.device,
            pci_dev.function,
            pci_dev.vendor_id,
            pci_dev.device_id,
            pci_dev.class_code,
            pci_dev.subclass,
            pci_dev.class_name()
        );
    }

    // Match and probe
    let mut matched = false;
    let mut i = 0;
    loop {
        let entry = id_table.add(i);
        if (*entry).vendor == 0
            && (*entry).device == 0
            && (*entry).subvendor == 0
            && (*entry).subdevice == 0
        {
            break;
        }
        for pci_dev in &devices {
            if ((*entry).vendor == 0xFFFF || (*entry).vendor as u16 == pci_dev.vendor_id)
                && ((*entry).device == 0xFFFF || (*entry).device as u16 == pci_dev.device_id)
            {
                crate::mesa_println!(
                    "[PCI] MATCH: entry[{}] ({:#06x}:{:#06x}) -> device {:02x}:{:02x}.{:x} ({})",
                    i,
                    (*entry).vendor,
                    (*entry).device,
                    pci_dev.bus,
                    pci_dev.device,
                    pci_dev.function,
                    pci_dev.class_name()
                );

                let dev_ptr = alloc_fake_pci_dev(pci_dev.bus, pci_dev.device, pci_dev.function);
                if dev_ptr.is_null() {
                    crate::mesa_println!("[PCI] Failed to allocate fake pci_dev, skipping probe");
                    matched = true;
                    break;
                }
                pci_driver_call_probe(drv, dev_ptr, entry);
                matched = true;
                break;
            }
        }
        i += 1;
    }

    // Fallback: if no vendor/device match found, try matching xHCI controllers by class code
    if !matched {
        for pci_dev in &devices {
            if pci_dev.class_code == 0x0c && pci_dev.subclass == 0x03 && pci_dev.prog_if == 0x30 {
                crate::mesa_println!(
                    "[PCI] FALLBACK: xHCI controller at {:02x}:{:02x}.{:x} ({:04x}:{:04x})",
                    pci_dev.bus,
                    pci_dev.device,
                    pci_dev.function,
                    pci_dev.vendor_id,
                    pci_dev.device_id
                );
                let dev_ptr = alloc_fake_pci_dev(pci_dev.bus, pci_dev.device, pci_dev.function);
                if dev_ptr.is_null() {
                    crate::mesa_println!("[PCI] Failed to allocate fake pci_dev, skipping probe");
                    continue;
                }
                // Use the first ID table entry for the call (safe since probe ignores ID if not needed)
                let entry = id_table;
                pci_driver_call_probe(drv, dev_ptr, entry);
            }
        }
    }
}

extern "C" fn __shim_pci_register_driver(drv: *mut u8) -> i32 {
    if drv.is_null() {
        return -22;
    }
    unsafe {
        let name_ptr = *(drv as *mut *mut u8);
        let id_table = *(drv.add(8) as *mut u64);
        let probe = *(drv.add(16) as *mut u64);
        let remove = *(drv.add(24) as *mut u64);
        let mut pdrv = PciDriver {
            addr: drv as u64,
            name: [0u8; 64],
            id_table,
            probe,
            remove,
        };
        if !name_ptr.is_null() {
            for i in 0..63 {
                let c = *name_ptr.add(i);
                pdrv.name[i] = c;
                if c == 0 {
                    break;
                }
            }
        }
        let name_str = core::str::from_utf8(&pdrv.name).unwrap_or("?");
        crate::mesa_println!("[PCI] register_driver: {} probe={:#x}", name_str, probe);
        PCI_DRIVERS.lock().push(pdrv.clone());
        pci_driver_match_and_probe(&PCI_DRIVERS.lock().last().unwrap());
    }
    0
}

extern "C" fn __shim_pci_unregister_driver(drv: *mut u8) {
    if drv.is_null() {
        return;
    }
    let key = drv as u64;
    let mut drivers = PCI_DRIVERS.lock();
    if let Some(pos) = drivers.iter().position(|d| d.addr == key) {
        drivers.remove(pos);
        // Don't call remove — no real devices were probed since
        // the MesaOS PCI subsystem doesn't enumerate HW for shim drivers.
    }
}

// ── IRQ Handling ──────────────────────────────────────────────
struct IrqHandler {
    handler: u64,
    dev_id: u64,
    name: [u8; 32],
    enabled: bool,
}

const MAX_IRQ: usize = 256;
static IRQ_HANDLERS: spin::Mutex<Vec<Option<IrqHandler>>> = spin::Mutex::new(Vec::new());

fn irq_ensure_slot(irq: usize) {
    let mut handlers = IRQ_HANDLERS.lock();
    while handlers.len() <= irq {
        handlers.push(None);
    }
}

extern "C" fn __shim_request_irq(
    irq: u32,
    handler: u64,
    _flags: u64,
    name: *const u8,
    dev_id: u64,
) -> i32 {
    if handler == 0 {
        return -22;
    }
    if (irq as usize) >= MAX_IRQ {
        return -22;
    }
    irq_ensure_slot(irq as usize);
    let mut handlers = IRQ_HANDLERS.lock();
    if handlers[irq as usize].is_some() {
        return -16;
    }
    let mut hname = [0u8; 32];
    if !name.is_null() {
        unsafe {
            for i in 0..31 {
                let c = *name.add(i);
                hname[i] = c;
                if c == 0 {
                    break;
                }
            }
        }
    }
    handlers[irq as usize] = Some(IrqHandler {
        handler,
        dev_id,
        name: hname,
        enabled: true,
    });
    let name_str = unsafe { core::str::from_utf8(&hname).unwrap_or("?") };
    crate::mesa_println!(
        "[IRQ] request_irq: irq={} handler={:#x} name={}",
        irq,
        handler,
        name_str
    );
    0
}

extern "C" fn __shim_free_irq(irq: u32, dev_id: u64) {
    if (irq as usize) >= MAX_IRQ {
        return;
    }
    let mut handlers = IRQ_HANDLERS.lock();
    if let Some(Some(h)) = handlers.get(irq as usize) {
        if h.dev_id == dev_id || dev_id == 0 {
            handlers[irq as usize] = None;
        }
    }
}

extern "C" fn __shim_enable_irq(irq: u32) {
    if (irq as usize) >= MAX_IRQ {
        return;
    }
    let mut handlers = IRQ_HANDLERS.lock();
    if let Some(Some(ref mut h)) = handlers.get_mut(irq as usize) {
        h.enabled = true;
    }
}

extern "C" fn __shim_disable_irq(irq: u32) {
    if (irq as usize) >= MAX_IRQ {
        return;
    }
    let mut handlers = IRQ_HANDLERS.lock();
    if let Some(Some(ref mut h)) = handlers.get_mut(irq as usize) {
        h.enabled = false;
    }
}

extern "C" fn __shim_synchronize_irq(_irq: u32) {}

/// Called by MesaOS's interrupt dispatcher to dispatch to registered Linux IRQ handlers.
/// Returns 1 if handled, 0 if no handler registered.
#[no_mangle]
pub extern "C" fn shim_dispatch_irq(irq: u32) -> i32 {
    if (irq as usize) >= MAX_IRQ {
        return 0;
    }
    let handlers = IRQ_HANDLERS.lock();
    if let Some(Some(ref h)) = handlers.get(irq as usize) {
        if !h.enabled {
            return 0;
        }
        let func: extern "C" fn(u32, u64) -> i32 = unsafe { core::mem::transmute(h.handler) };
        func(irq, h.dev_id)
    } else {
        0
    }
}

// ── Tasklets ──────────────────────────────────────────────────
struct TaskletEntry {
    addr: u64,
    data: u64,
    pending: bool,
}

static TASKLETS: spin::Mutex<Vec<TaskletEntry>> = spin::Mutex::new(Vec::new());

extern "C" fn __shim_tasklet_init(func: u64, data: u64) -> u64 {
    let mut ts = TASKLETS.lock();
    let id = ts.len() as u64;
    ts.push(TaskletEntry {
        addr: func,
        data,
        pending: false,
    });
    id
}

extern "C" fn __shim_tasklet_schedule(tasklet: u64) {
    if (tasklet as usize) >= TASKLETS.lock().len() {
        return;
    }
    TASKLETS.lock()[tasklet as usize].pending = true;
}

extern "C" fn __shim_tasklet_hi_schedule(tasklet: u64) {
    __shim_tasklet_schedule(tasklet);
}

extern "C" fn __shim_tasklet_kill(tasklet: u64) {
    if (tasklet as usize) >= TASKLETS.lock().len() {
        return;
    }
    TASKLETS.lock()[tasklet as usize].pending = false;
}

extern "C" fn __shim_raise_softirq(_nr: u32) {}

/// Called periodically by the kernel to run pending tasklets.
#[no_mangle]
pub extern "C" fn shim_run_tasklets() {
    let mut ts = TASKLETS.lock();
    for i in 0..ts.len() {
        if ts[i].pending {
            ts[i].pending = false;
            let func: extern "C" fn(u64) = unsafe { core::mem::transmute(ts[i].addr) };
            func(ts[i].data);
        }
    }
}

// ── Networking Integration ────────────────────────────────────
extern "C" fn __shim_dev_queue_xmit(skb: *mut u8) -> i32 {
    if skb.is_null() {
        return -22;
    }
    let len = skb_rd32(skb, 0x20);
    let data_ptr = skb_rd64(skb, 0x08);
    if !data_ptr.is_null() && len > 0 {
        let packet = unsafe { core::slice::from_raw_parts(data_ptr, len as usize) };
        if let Some(mac) = crate::drivers::net::virtio_net::get_mac() {
            let _ = crate::drivers::net::virtio_net::send_packet(packet);
        } else if let Some(mac) = crate::drivers::net::rtl8139::get_mac() {
            let _ = crate::drivers::net::rtl8139::send_packet(packet);
        }
    }
    unsafe {
        kfree(skb);
    }
    0
}

extern "C" fn __shim_netif_carrier_on(dev: *mut u8) {
    if !dev.is_null() {
        crate::mesa_println!("[NET] netif_carrier_on: {:p}", dev);
    }
}

extern "C" fn __shim_netif_carrier_off(dev: *mut u8) {
    if !dev.is_null() {
        crate::mesa_println!("[NET] netif_carrier_off: {:p}", dev);
    }
}

extern "C" fn __shim_usb_alloc_urb(_iso_packets: i32, _mem_flags: u32) -> *mut u8 {
    unsafe { kzalloc(256, 0) }
}

extern "C" fn __shim_usb_free_urb(_urb: *mut u8) {
    unsafe {
        kfree(_urb);
    }
}

// ── USB Device Registry ──────────────────────────────────────
// Maps fake usb_device addresses -> xHCI controller + slot_id for routing transfers.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnumStage {
    Idle = 0,
    PortReset = 1,
    EnableSlot = 2,
    AddressDevice = 3,
    DeviceDescriptor = 4,
    ConfigDescriptor = 5,
    BulkConfig = 6,
    StorageProbe = 7,
    Complete = 8,
    Error = 255,
}

impl EnumStage {
    pub fn name(self) -> &'static str {
        match self {
            EnumStage::Idle => "Idle",
            EnumStage::PortReset => "PortReset",
            EnumStage::EnableSlot => "EnableSlot",
            EnumStage::AddressDevice => "AddressDevice",
            EnumStage::DeviceDescriptor => "DeviceDescriptor",
            EnumStage::ConfigDescriptor => "ConfigDescriptor",
            EnumStage::BulkConfig => "BulkConfig",
            EnumStage::StorageProbe => "StorageProbe",
            EnumStage::Complete => "Complete",
            EnumStage::Error => "Error",
        }
    }
}

#[derive(Clone, Copy)]
pub struct EnumDebugInfo {
    pub stage: EnumStage,
    pub error_msg: [u8; 128],
    pub error_len: u8,
    pub tick_start: u64,
    pub tick_stage: u64,
    pub timed_out: bool,
    pub retry_count: u32,
}

impl EnumDebugInfo {
    pub fn new() -> Self {
        Self {
            stage: EnumStage::Idle,
            error_msg: [0u8; 128],
            error_len: 0,
            tick_start: 0,
            tick_stage: 0,
            timed_out: false,
            retry_count: 0,
        }
    }

    pub fn set_stage(&mut self, stage: EnumStage) {
        self.stage = stage;
        self.tick_stage = crate::curr_arch::get_ticks();
        if stage == EnumStage::PortReset && self.tick_start == 0 {
            self.tick_start = self.tick_stage;
        }
        self.clear_error();
    }

    pub fn set_error(&mut self, msg: &str) {
        let bytes = msg.as_bytes();
        let len = core::cmp::min(bytes.len(), 127);
        self.error_msg[..len].copy_from_slice(&bytes[..len]);
        self.error_msg[len] = 0;
        self.error_len = len as u8;
        self.stage = EnumStage::Error;
    }

    pub fn clear_error(&mut self) {
        self.error_msg = [0u8; 128];
        self.error_len = 0;
    }

    pub fn error_string(&self) -> &str {
        let end = self.error_msg.iter().position(|&c| c == 0).unwrap_or(0);
        core::str::from_utf8(&self.error_msg[..end]).unwrap_or("(invalid utf8)")
    }
}

struct UsbDeviceRecord {
    ctrl_idx: usize,
    slot_id: u32,
    port: u32,
    speed: u32,
    vendor_id: u16,
    product_id: u16,
    device_class: u8,
    device_subclass: u8,
    device_protocol: u8,
}

static USB_DEVICE_RECORDS: spin::Mutex<BTreeMap<u64, UsbDeviceRecord>> =
    spin::Mutex::new(BTreeMap::new());
static USB_DEVICE_ADDR_COUNTER: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0x2000000);

fn usb_register_device(
    ctrl_idx: usize,
    slot_id: u32,
    port: u32,
    speed: u32,
    dev_desc: &[u8; 18],
) -> *mut u8 {
    let addr = USB_DEVICE_ADDR_COUNTER.fetch_add(4096, core::sync::atomic::Ordering::Relaxed);
    let record = UsbDeviceRecord {
        ctrl_idx,
        slot_id,
        port,
        speed,
        vendor_id: u16::from_le_bytes([dev_desc[8], dev_desc[9]]),
        product_id: u16::from_le_bytes([dev_desc[10], dev_desc[11]]),
        device_class: dev_desc[4],
        device_subclass: dev_desc[5],
        device_protocol: dev_desc[6],
    };
    crate::mesa_println!(
        "[USB] Registered device: addr={:#x} ctrl={} slot={} vid={:#x} pid={:#x}",
        addr,
        ctrl_idx,
        slot_id,
        record.vendor_id,
        record.product_id
    );
    USB_DEVICE_RECORDS.lock().insert(addr, record);
    addr as *mut u8
}

fn usb_find_device(dev: *mut u8) -> Option<(usize, u32)> {
    if dev.is_null() {
        return None;
    }
    let records = USB_DEVICE_RECORDS.lock();
    records.get(&(dev as u64)).map(|r| (r.ctrl_idx, r.slot_id))
}

fn usb_deregister_device(dev: *mut u8) {
    if !dev.is_null() {
        USB_DEVICE_RECORDS.lock().remove(&(dev as u64));
    }
}

// ── USB driver info ──────────────────────────────────────────
#[derive(Clone)]
struct UsbDriverInfo {
    addr: u64,
    name: [u8; 64],
    id_table: u64,
    probe: u64,
    disconnect: u64,
}

static USB_DRIVERS: spin::Mutex<Vec<UsbDriverInfo>> = spin::Mutex::new(Vec::new());
static USB_IFACE_MAP: spin::Mutex<BTreeMap<u64, i32>> = spin::Mutex::new(BTreeMap::new());
static USB_INTF_TO_DEV: spin::Mutex<BTreeMap<u64, u64>> = spin::Mutex::new(BTreeMap::new());
static USB_INTF_DATA: spin::Mutex<BTreeMap<u64, u64>> = spin::Mutex::new(BTreeMap::new());

fn usb_pipe_dir_in(pipe: u32) -> bool {
    usb_pipe_dir(pipe)
}

extern "C" fn __shim_usb_submit_urb(urb_ptr: *mut u8, _mem_flags: u32) -> i32 {
    if urb_ptr.is_null() {
        return -22;
    }
    unsafe {
        // Linux 6.x URB struct offsets (x86_64):
        //   dev:            offset 16 (u64)
        //   pipe:           offset 24 (u32)
        //   status:         offset 28 (i32)
        //   transfer_buffer: offset 40 (u64)
        //   transfer_length: offset 56 (i32)
        //   actual_length:  offset 60 (i32)
        //   setup_packet:   offset 64 (u64)
        //   complete:       offset 96 (u64 function ptr)
        //   context:        offset 88 (u64)
        let usb_dev = *(urb_ptr.add(16) as *const u64) as *mut u8;
        let pipe = *(urb_ptr.add(24) as *const u32);
        let transfer_buf = *(urb_ptr.add(40) as *const u64) as *mut u8;
        let transfer_len = *(urb_ptr.add(56) as *const i32);
        let setup_pkt = *(urb_ptr.add(64) as *const u64) as *mut u8;
        let complete_fn = *(urb_ptr.add(96) as *const u64);

        let ptype = usb_pipe_type(pipe);
        let endpoint_num = usb_pipe_endpoint(pipe);
        let dir_in = usb_pipe_dir(pipe);

        let result = if ptype == 2 {
            // Control transfer
            let mut setup: [u8; 8] = [0u8; 8];
            if !setup_pkt.is_null() {
                core::ptr::copy_nonoverlapping(setup_pkt, setup.as_mut_ptr(), 8);
            }
            let records = USB_DEVICE_RECORDS.lock();
            if let Some(rec) = records.get(&(usb_dev as u64)) {
                let (ctrl_idx, slot_id) = (rec.ctrl_idx, rec.slot_id);
                drop(records);
                let mut controllers = XHCI_CONTROLLERS.lock();
                if ctrl_idx < controllers.len() {
                    xhci_control_transfer(
                        &mut controllers[ctrl_idx],
                        slot_id,
                        &setup,
                        transfer_buf,
                        transfer_len as u16,
                        dir_in,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else if ptype == 3 {
            // Bulk transfer
            let epid = if endpoint_num == 0 {
                1u32
            } else {
                ((endpoint_num as u32) * 2) + if dir_in { 1 } else { 0 }
            };
            let records = USB_DEVICE_RECORDS.lock();
            if let Some(rec) = records.get(&(usb_dev as u64)) {
                let (ctrl_idx, slot_id) = (rec.ctrl_idx, rec.slot_id);
                drop(records);
                let mut controllers = XHCI_CONTROLLERS.lock();
                if ctrl_idx < controllers.len() {
                    xhci_bulk_transfer(
                        &mut controllers[ctrl_idx],
                        slot_id,
                        epid,
                        transfer_buf,
                        transfer_len as u32,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            crate::mesa_println!("[USB] usb_submit_urb: unsupported pipe type {}", ptype);
            false
        };

        if result {
            *(urb_ptr.add(28) as *mut i32) = 0; // status = 0 (success)
            *(urb_ptr.add(60) as *mut i32) = transfer_len; // actual_length
            if complete_fn != 0 {
                let complete: extern "C" fn(*mut u8) = core::mem::transmute(complete_fn);
                complete(urb_ptr);
            }
            return 0;
        }
        *(urb_ptr.add(28) as *mut i32) = -5; // status = -EIO
        if complete_fn != 0 {
            let complete: extern "C" fn(*mut u8) = core::mem::transmute(complete_fn);
            complete(urb_ptr);
        }
    }
    -5
}

extern "C" fn __shim_usb_kill_urb(_urb_ptr: *mut u8) {}

extern "C" fn __shim_usb_control_msg(
    dev: *mut u8,
    pipe: u32,
    request: u8,
    requesttype: u8,
    value: u16,
    index: u16,
    data: *mut u8,
    size: u16,
    _timeout: i32,
) -> i32 {
    if dev.is_null() {
        return -22;
    }
    let dir_in = (pipe & 0x10) != 0;
    let setup_pkt = [
        requesttype,
        request,
        value as u8,
        (value >> 8) as u8,
        index as u8,
        (index >> 8) as u8,
        size as u8,
        (size >> 8) as u8,
    ];
    let records = USB_DEVICE_RECORDS.lock();
    if let Some(rec) = records.get(&(dev as u64)) {
        let mut controllers = XHCI_CONTROLLERS.lock();
        if rec.ctrl_idx < controllers.len() {
            let ok = unsafe {
                xhci_control_transfer(
                    &mut controllers[rec.ctrl_idx],
                    rec.slot_id,
                    &setup_pkt,
                    data,
                    size,
                    dir_in,
                )
            };
            if ok {
                return size as i32;
            }
        }
    }
    crate::mesa_println!(
        "[USB] usb_control_msg FAILED: dev={:p} rtype={:#x} rq={}",
        dev,
        requesttype,
        request
    );
    -5 // -EIO
}

extern "C" fn __shim_usb_bulk_msg(
    dev: *mut u8,
    pipe: u32,
    data: *mut u8,
    len: i32,
    actual: *mut i32,
    _timeout: i32,
) -> i32 {
    if dev.is_null() || data.is_null() || len <= 0 {
        return -22;
    }
    let endpoint_num = usb_pipe_endpoint(pipe);
    let dir_in = usb_pipe_dir(pipe);
    // Map (endpoint_number, direction) -> xHCI endpoint ID
    // EP0=1, EP1 OUT=2, EP1 IN=3, EP2 OUT=4, EP2 IN=5, ...
    let epid = if endpoint_num == 0 {
        1u32
    } else {
        ((endpoint_num as u32) * 2) + if dir_in { 1 } else { 0 }
    };
    let records = USB_DEVICE_RECORDS.lock();
    if let Some(rec) = records.get(&(dev as u64)) {
        let (ctrl_idx, slot_id) = (rec.ctrl_idx, rec.slot_id);
        drop(records);
        let mut controllers = XHCI_CONTROLLERS.lock();
        if ctrl_idx < controllers.len() {
            let ok = unsafe {
                xhci_bulk_transfer(&mut controllers[ctrl_idx], slot_id, epid, data, len as u32)
            };
            if ok {
                if !actual.is_null() {
                    unsafe {
                        *actual = len;
                    }
                }
                return len;
            }
        }
    }
    crate::mesa_println!(
        "[USB] usb_bulk_msg FAILED: dev={:p} pipe={:#x} len={}",
        dev,
        pipe,
        len
    );
    -5
}

extern "C" fn __shim_usb_reset_device(dev: *mut u8) -> i32 {
    if dev.is_null() {
        return -22;
    }
    crate::mesa_println!("[USB] usb_reset_device: {:p}", dev);
    0
}

extern "C" fn __shim_usb_get_descriptor(
    dev: *mut u8,
    desc_type: u8,
    desc_index: u8,
    buf: *mut u8,
    size: i32,
) -> i32 {
    if dev.is_null() || buf.is_null() || size <= 0 {
        return -22;
    }
    let setup_pkt = [
        USB_DIR_IN,             // bmRequestType = device-to-host, standard, device
        USB_REQ_GET_DESCRIPTOR, // bRequest
        desc_type,              // wValue low = descriptor type
        desc_index,             // wValue high = descriptor index
        0,                      // wIndex low = 0 (language ID)
        0,                      // wIndex high
        size as u8,             // wLength low
        (size >> 8) as u8,      // wLength high
    ];
    let records = USB_DEVICE_RECORDS.lock();
    if let Some(rec) = records.get(&(dev as u64)) {
        let mut controllers = XHCI_CONTROLLERS.lock();
        if rec.ctrl_idx < controllers.len() {
            let ok = unsafe {
                xhci_control_transfer(
                    &mut controllers[rec.ctrl_idx],
                    rec.slot_id,
                    &setup_pkt,
                    buf,
                    size as u16,
                    true,
                )
            };
            if ok {
                return size;
            }
        }
    }
    crate::mesa_println!(
        "[USB] usb_get_descriptor FAILED: dev={:p} type={:#x}",
        dev,
        desc_type
    );
    -5 // -EIO
}

extern "C" fn __shim_usb_set_interface(dev: *mut u8, interface: i32, alternate: i32) -> i32 {
    if dev.is_null() {
        return -22;
    }
    let key = (dev as u64) << 32 | (interface as u64 & 0xFFFFFFFF);
    USB_IFACE_MAP.lock().insert(key, alternate);
    0
}

// ── USB Device ID matching ──────────────────────────────────
const USB_DEVICE_ID_SIZE: usize = 24;

const USB_DEVICE_ID_MATCH_VENDOR: u16 = 0x0001;
const USB_DEVICE_ID_MATCH_PRODUCT: u16 = 0x0002;
const USB_DEVICE_ID_MATCH_DEV_LO: u16 = 0x0004;
const USB_DEVICE_ID_MATCH_DEV_HI: u16 = 0x0008;
const USB_DEVICE_ID_MATCH_DEV_CLASS: u16 = 0x0010;
const USB_DEVICE_ID_MATCH_DEV_SUBCLASS: u16 = 0x0020;
const USB_DEVICE_ID_MATCH_DEV_PROTOCOL: u16 = 0x0040;
const USB_DEVICE_ID_MATCH_INT_CLASS: u16 = 0x0080;
const USB_DEVICE_ID_MATCH_INT_SUBCLASS: u16 = 0x0100;
const USB_DEVICE_ID_MATCH_INT_PROTOCOL: u16 = 0x0200;
const USB_DEVICE_ID_MATCH_INT_NUMBER: u16 = 0x0400;

unsafe fn usb_id_match_entry(entry: *const u8, vendor_id: u16, product_id: u16) -> bool {
    let match_flags = u16::from_le(*(entry as *const u16));
    if match_flags == 0 {
        return false; // end of table
    }
    let id_vendor = u16::from_le(*(entry.add(2) as *const u16));
    let id_product = u16::from_le(*(entry.add(4) as *const u16));
    if (match_flags & USB_DEVICE_ID_MATCH_VENDOR) != 0 && id_vendor != vendor_id {
        return false;
    }
    if (match_flags & USB_DEVICE_ID_MATCH_PRODUCT) != 0 && id_product != product_id {
        return false;
    }
    true
}

// Removed usb_driver_probe_device, usb_iterate_controllers_and_probe, and usb_driver_probe_device_ext
// since they depend on the native XHCI driver.

// ── USB driver registration / deregistration ────────────────
extern "C" fn __shim_usb_register_driver(
    driver: *mut u8,
    _module: *mut u8,
    name: *const u8,
) -> i32 {
    if driver.is_null() {
        return -22;
    }
    unsafe {
        let name_ptr = *(driver as *mut *mut u8);
        // struct usb_driver offset for probe/disconnect/id_table (Linux 6.x):
        // probe at 8, disconnect at 16, id_table at 72
        let probe = *(driver.add(8) as *mut u64);
        let disconnect = *(driver.add(16) as *mut u64);
        let id_table = *(driver.add(72) as *mut u64);

        let drv_key = driver as u64;
        {
            let mut drvs = USB_DRIVERS.lock();
            if drvs.iter().any(|d| d.addr == drv_key) {
                return 0;
            }
        }

        let mut name_buf = [0u8; 64];
        if !name_ptr.is_null() {
            for i in 0..63 {
                let c = *name_ptr.add(i);
                name_buf[i] = c;
                if c == 0 {
                    break;
                }
            }
        }
        let name_str = core::str::from_utf8(&name_buf).unwrap_or("?");
        crate::mesa_println!(
            "[USB] register_driver: {} probe={:#x} id_table={:#x}",
            name_str,
            probe,
            id_table
        );

        let drv_info = UsbDriverInfo {
            addr: drv_key,
            name: name_buf,
            id_table,
            probe,
            disconnect,
        };

        // Note: No more usb_iterate_controllers_and_probe because native XHCI is gone.
        // We will need a generic usb core mechanism if we want to enumerate.
        USB_DRIVERS.lock().push(drv_info);
    }
    0
}

extern "C" fn __shim_usb_deregister(driver: *mut u8) {
    if driver.is_null() {
        return;
    }
    let key = driver as u64;
    let mut drvs = USB_DRIVERS.lock();
    if let Some(pos) = drvs.iter().position(|d| d.addr == key) {
        let drv_info = drvs.remove(pos);
        if drv_info.disconnect != 0 {
            // Call disconnect on all interfaces - iterate all devices
            let disconnect_fn: extern "C" fn(*mut u8) =
                unsafe { core::mem::transmute(drv_info.disconnect) };
            // Simplified: just call disconnect with null for now
            disconnect_fn(core::ptr::null_mut());
        }
        crate::mesa_println!("[USB] deregister: {:p}", driver);
    }
}

// ── USB interface helpers (exported symbols for MesaOS-compiled drivers) ──
extern "C" fn __shim_interface_to_usbdev(intf: *mut u8) -> *mut u8 {
    if intf.is_null() {
        return core::ptr::null_mut();
    }
    let map = USB_INTF_TO_DEV.lock();
    let dev = map.get(&(intf as u64)).copied().unwrap_or(0);
    dev as *mut u8
}

extern "C" fn __shim_usb_set_intfdata(intf: *mut u8, data: *mut u8) {
    if !intf.is_null() {
        USB_INTF_DATA.lock().insert(intf as u64, data as u64);
    }
}

extern "C" fn __shim_usb_get_intfdata(intf: *mut u8) -> *mut u8 {
    if intf.is_null() {
        return core::ptr::null_mut();
    }
    let map = USB_INTF_DATA.lock();
    let data = map.get(&(intf as u64)).copied().unwrap_or(0);
    data as *mut u8
}

extern "C" fn __shim_usb_ifnum_to_if(dev: *mut u8, ifnum: i32) -> *mut u8 {
    if dev.is_null() {
        return core::ptr::null_mut();
    }
    unsafe { dev.add(0x100 + (ifnum as usize) * 64) }
}

fn usb_pipe_type(pipe: u32) -> u32 {
    (pipe >> 8) & 0xF
}
fn usb_pipe_dir(pipe: u32) -> bool {
    (pipe & 0x10) != 0
}
fn usb_pipe_endpoint(pipe: u32) -> i32 {
    (pipe & 0xF) as i32
}

fn usb_make_pipe(endpoint: i32, ptype: u32, dir_in: bool) -> u32 {
    ((endpoint as u32) & 0xF) | (if dir_in { 0x10 } else { 0 }) | ((ptype & 0xF) << 8)
}

extern "C" fn __shim_usb_rcvctrlpipe(_dev: *mut u8, endpoint: i32) -> u32 {
    usb_make_pipe(endpoint, 2, true) // PIPE_CONTROL=2, IN
}

extern "C" fn __shim_usb_sndctrlpipe(_dev: *mut u8, endpoint: i32) -> u32 {
    usb_make_pipe(endpoint, 2, false) // PIPE_CONTROL=2, OUT
}

extern "C" fn __shim_usb_rcvbulkpipe(_dev: *mut u8, endpoint: i32) -> u32 {
    usb_make_pipe(endpoint, 3, true) // PIPE_BULK=3, IN
}

extern "C" fn __shim_usb_sndbulkpipe(_dev: *mut u8, endpoint: i32) -> u32 {
    usb_make_pipe(endpoint, 3, false) // PIPE_BULK=3, OUT
}

extern "C" fn __shim_usb_rcvintpipe(_dev: *mut u8, endpoint: i32) -> u32 {
    usb_make_pipe(endpoint, 1, true) // PIPE_INTERRUPT=1, IN
}

extern "C" fn __shim_usb_sndintpipe(_dev: *mut u8, endpoint: i32) -> u32 {
    usb_make_pipe(endpoint, 1, false) // PIPE_INTERRUPT=1, OUT
}

extern "C" fn __shim_usb_maxpacket(dev: *mut u8, pipe: u32) -> i32 {
    if dev.is_null() {
        return 64;
    }
    let endpoint = usb_pipe_endpoint(pipe);
    let dir_in = usb_pipe_dir(pipe);
    unsafe {
        // usb_host_endpoint at offset 0x28 (simplified - scans for matching endpoint)
        let ep_descs = dev.add(0x70) as *const u8;
        for i in 0..16 {
            let d = *ep_descs.add(i as usize * 7);
            if d == 0 {
                break;
            }
            let ep = d & 0xF;
            let dir = (d & 0x80) != 0;
            if ep == endpoint as u8 && ((dir == dir_in) || (d & 0x80) == 0) {
                let wMaxPktSize = *(ep_descs.add(i as usize * 7 + 4) as *const u16);
                return u16::from_le(wMaxPktSize) as i32;
            }
        }
    }
    64
}

extern "C" fn __shim_usb_get_dev(dev: *mut u8) -> *mut u8 {
    dev
}

extern "C" fn __shim_usb_put_dev(_dev: *mut u8) {}

extern "C" fn __shim_msleep(ms: u32) {
    crate::linux::msleep(ms as u64);
}

extern "C" fn __shim_mdelay(ms: u32) {
    crate::linux::mdelay(ms as u64);
}

extern "C" fn __shim_udelay(us: u32) {
    for _ in 0..us * 100 {
        core::hint::spin_loop();
    }
}

extern "C" fn __shim_ssleep(s: u32) {
    crate::linux::msleep(s as u64 * 1000);
}

extern "C" fn __shim_schedule() {
    crate::scheduler::yield_now();
}

extern "C" fn __shim_wmb() {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

extern "C" fn __shim_rmb() {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

extern "C" fn __shim_mb() {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

extern "C" fn __shim_barrier() {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

extern "C" fn __shim_get_cycles() -> u64 {
    crate::curr_arch::get_ticks()
}

static mut THIS_MODULE_DATA: [u8; 1024] = [0u8; 1024];

extern "C" fn __shim_this_module() -> *mut u8 {
    unsafe { THIS_MODULE_DATA.as_mut_ptr() }
}

extern "C" fn __shim_list_add(new: *mut u8, head: *mut u8) {
    let new = new as *mut crate::linux::list::list_head;
    let head = head as *mut crate::linux::list::list_head;
    unsafe {
        (*head).add(new);
    }
}

extern "C" fn __shim_list_del(entry: *mut u8) {
    let entry = entry as *mut crate::linux::list::list_head;
    unsafe {
        (*entry).del();
    }
}

extern "C" fn __shim_list_empty(head: *mut u8) -> i32 {
    let head = head as *mut crate::linux::list::list_head;
    unsafe {
        if (*head).next == head {
            1
        } else {
            0
        }
    }
}

extern "C" fn __shim_list_for_each(pos: *mut *mut u8, head: *mut u8) {
    unsafe {
        *pos = (*head.cast::<*mut u8>()).offset(0) as *mut u8;
    }
}

fn atomic_op<F: Fn(&mut AtomicI32) -> i32>(v: *mut u8, op: F) -> i32 {
    op(unsafe { &mut *(v as *mut AtomicI32) })
}

extern "C" fn __shim_atomic_set(v: *mut u8, i: i32) {
    unsafe {
        *(v as *mut AtomicI32) = AtomicI32::new(i);
    }
}

extern "C" fn __shim_atomic_read(v: *mut u8) -> i32 {
    unsafe { (*(v as *mut AtomicI32)).load(Ordering::Relaxed) }
}

extern "C" fn __shim_atomic_add(i: i32, v: *mut u8) {
    unsafe {
        (*(v as *mut AtomicI32)).fetch_add(i, Ordering::Relaxed);
    }
}

extern "C" fn __shim_atomic_sub(i: i32, v: *mut u8) {
    unsafe {
        (*(v as *mut AtomicI32)).fetch_sub(i, Ordering::Relaxed);
    }
}

extern "C" fn __shim_atomic_inc(v: *mut u8) {
    unsafe {
        (*(v as *mut AtomicI32)).fetch_add(1, Ordering::Relaxed);
    }
}

extern "C" fn __shim_atomic_dec(v: *mut u8) {
    unsafe {
        (*(v as *mut AtomicI32)).fetch_sub(1, Ordering::Relaxed);
    }
}

extern "C" fn __shim_atomic_add_return(i: i32, v: *mut u8) -> i32 {
    unsafe {
        (*(v as *mut AtomicI32))
            .fetch_add(i, Ordering::Acquire)
            .wrapping_add(i)
    }
}

extern "C" fn __shim_atomic_sub_return(i: i32, v: *mut u8) -> i32 {
    unsafe {
        (*(v as *mut AtomicI32))
            .fetch_sub(i, Ordering::Acquire)
            .wrapping_sub(i)
    }
}

extern "C" fn __shim_atomic_inc_return(v: *mut u8) -> i32 {
    unsafe {
        (*(v as *mut AtomicI32))
            .fetch_add(1, Ordering::Acquire)
            .wrapping_add(1)
    }
}

extern "C" fn __shim_atomic_dec_return(v: *mut u8) -> i32 {
    unsafe {
        (*(v as *mut AtomicI32))
            .fetch_sub(1, Ordering::Acquire)
            .wrapping_sub(1)
    }
}

/// MesaOS sk_buff layout:
/// [0x00]: head *mut u8  — start of data buffer
/// [0x08]: data *mut u8  — current data start pointer
/// [0x10]: tail *mut u8  — current data end pointer
/// [0x18]: end  *mut u8  — end of data buffer
/// [0x20]: len  u32      — current data length
/// [0x24]: truesize u32  — total allocated size
/// [0x28]: dev  *mut u8  — net_device pointer
/// [0x30]: data buffer begins here

const SKB_HDR_SIZE: usize = 0x30;

fn skb_rd64(skb: *mut u8, off: usize) -> *mut u8 {
    unsafe { *(skb.add(off) as *mut *mut u8) }
}
fn skb_wr64(skb: *mut u8, off: usize, val: *mut u8) {
    unsafe {
        *(skb.add(off) as *mut *mut u8) = val;
    }
}
fn skb_rd32(skb: *mut u8, off: usize) -> u32 {
    unsafe { *(skb.add(off) as *mut u32) }
}
fn skb_wr32(skb: *mut u8, off: usize, val: u32) {
    unsafe {
        *(skb.add(off) as *mut u32) = val;
    }
}

extern "C" fn __shim_dev_alloc_skb(len: u32) -> *mut u8 {
    let total = SKB_HDR_SIZE + len as usize;
    let skb = unsafe { kzalloc(total, 0) };
    if skb.is_null() {
        return skb;
    }
    let buf = unsafe { skb.add(SKB_HDR_SIZE) };
    skb_wr64(skb, 0x00, buf); // head
    skb_wr64(skb, 0x08, buf); // data
    skb_wr64(skb, 0x10, buf); // tail
    skb_wr64(skb, 0x18, unsafe { buf.add(len as usize) }); // end
    skb_wr32(skb, 0x20, 0); // len
    skb_wr32(skb, 0x24, total as u32); // truesize
    skb_wr64(skb, 0x28, core::ptr::null_mut()); // dev
    skb
}

extern "C" fn __shim_kfree_skb(skb: *mut u8) {
    if !skb.is_null() {
        unsafe {
            kfree(skb);
        }
    }
}

extern "C" fn __shim_skb_put(skb: *mut u8, len: u32) -> *mut u8 {
    if skb.is_null() || len == 0 {
        return core::ptr::null_mut();
    }
    let tail = skb_rd64(skb, 0x10);
    let new_tail = unsafe { tail.add(len as usize) };
    let end = skb_rd64(skb, 0x18);
    if new_tail > end {
        return core::ptr::null_mut();
    }
    skb_wr64(skb, 0x10, new_tail);
    let cur_len = skb_rd32(skb, 0x20);
    skb_wr32(skb, 0x20, cur_len + len);
    tail
}

extern "C" fn __shim_skb_push(skb: *mut u8, len: u32) -> *mut u8 {
    if skb.is_null() || len == 0 {
        return core::ptr::null_mut();
    }
    let data = skb_rd64(skb, 0x08);
    let head = skb_rd64(skb, 0x00);
    if data < unsafe { head.add(len as usize) } {
        return core::ptr::null_mut();
    }
    let new_data = unsafe { data.sub(len as usize) };
    skb_wr64(skb, 0x08, new_data);
    let cur_len = skb_rd32(skb, 0x20);
    skb_wr32(skb, 0x20, cur_len + len);
    new_data
}

extern "C" fn __shim_skb_reserve(skb: *mut u8, len: u32) {
    if skb.is_null() || len == 0 {
        return;
    }
    let data = skb_rd64(skb, 0x08);
    let new_data = unsafe { data.add(len as usize) };
    let tail = skb_rd64(skb, 0x10);
    skb_wr64(skb, 0x08, new_data);
    skb_wr64(skb, 0x10, unsafe { tail.add(len as usize) });
}

extern "C" fn __shim_skb_copy_to_linear_data(skb: *mut u8, src: *const u8, len: u32) {
    if skb.is_null() || src.is_null() || len == 0 {
        return;
    }
    let data = skb_rd64(skb, 0x08);
    unsafe {
        core::ptr::copy_nonoverlapping(src, data, len as usize);
    }
}

extern "C" fn __shim_skb_copy_from_linear_data(skb: *mut u8, dst: *mut u8, len: u32) {
    if skb.is_null() || dst.is_null() || len == 0 {
        return;
    }
    let data = skb_rd64(skb, 0x08);
    unsafe {
        core::ptr::copy_nonoverlapping(data, dst, len as usize);
    }
}

extern "C" fn __shim_eth_type_trans(skb: *mut u8, dev: *mut u8) -> u16 {
    if skb.is_null() {
        return 0;
    }
    let data = skb_rd64(skb, 0x08);
    if data.is_null() {
        return 0;
    }
    unsafe {
        let eth_type = *(data.add(12) as *const u16);
        u16::from_be(eth_type)
    }
}

static NET_DEVICES: spin::Mutex<Vec<u64>> = spin::Mutex::new(Vec::new());
static NET_QUEUE_STATE: spin::Mutex<BTreeMap<u64, bool>> = spin::Mutex::new(BTreeMap::new());

extern "C" fn __shim_netif_rx(skb: *mut u8) -> i32 {
    if skb.is_null() {
        return -22;
    }
    crate::mesa_println!("[NET] netif_rx: skb={:p} len={}", skb, skb_rd32(skb, 0x20));
    unsafe {
        kfree(skb);
    }
    0
}

extern "C" fn __shim_netif_receive_skb(skb: *mut u8) -> i32 {
    if skb.is_null() {
        return -22;
    }
    crate::mesa_println!(
        "[NET] netif_receive_skb: skb={:p} len={}",
        skb,
        skb_rd32(skb, 0x20)
    );
    unsafe {
        kfree(skb);
    }
    0
}

extern "C" fn __shim_netif_start_queue(dev: *mut u8) {
    __shim_netif_wake_queue(dev);
}

extern "C" fn __shim_netif_wake_queue(dev: *mut u8) {
    if !dev.is_null() {
        NET_QUEUE_STATE.lock().insert(dev as u64, true);
        crate::mesa_println!("[NET] netif_wake_queue: {:p}", dev);
    }
}

extern "C" fn __shim_netif_stop_queue(dev: *mut u8) {
    if !dev.is_null() {
        NET_QUEUE_STATE.lock().insert(dev as u64, false);
        crate::mesa_println!("[NET] netif_stop_queue: {:p}", dev);
    }
}

extern "C" fn __shim_register_netdev(dev: *mut u8) -> i32 {
    if dev.is_null() {
        return -22;
    }
    let key = dev as u64;
    {
        let mut net = NET_DEVICES.lock();
        if !net.contains(&key) {
            net.push(key);
        }
    }
    crate::mesa_println!(
        "[NET] register_netdev: {:p} (total {})",
        dev,
        NET_DEVICES.lock().len()
    );
    0
}

extern "C" fn __shim_unregister_netdev(dev: *mut u8) {
    if !dev.is_null() {
        NET_DEVICES.lock().retain(|&d| d != dev as u64);
        NET_QUEUE_STATE.lock().remove(&(dev as u64));
        crate::mesa_println!("[NET] unregister_netdev: {:p}", dev);
    }
}

extern "C" fn __shim_alloc_etherdev(sizeof_priv: i32) -> *mut u8 {
    let total = 2048 + sizeof_priv.max(0) as usize;
    unsafe { kzalloc(total, 0) }
}

extern "C" fn __shim_netif_device_attach(dev: *mut u8) {
    if !dev.is_null() {
        __shim_netif_wake_queue(dev);
        crate::mesa_println!("[NET] netif_device_attach: {:p}", dev);
    }
}

extern "C" fn __shim_netif_device_detach(dev: *mut u8) {
    if !dev.is_null() {
        __shim_netif_stop_queue(dev);
        crate::mesa_println!("[NET] netif_device_detach: {:p}", dev);
    }
}

extern "C" fn __shim_free_netdev(dev: *mut u8) {
    if !dev.is_null() {
        unsafe {
            kfree(dev);
        }
    }
}

// ── Firmware ──────────────────────────────────────────────────
unsafe fn load_firmware_to_buf(fname: &str) -> Result<*mut u8, ()> {
    let paths = [
        alloc::format!("/lib/firmware/{}", fname),
        alloc::format!("/bin/lib/firmware/{}", fname),
    ];
    let chosen = paths
        .iter()
        .find(|p| crate::fs::exists(p))
        .map(|p| p.clone());
    let path = match chosen {
        Some(ref p) => p.clone(),
        None => {
            crate::mesa_println!(
                "[FW] '{}' not found (tried /lib/firmware/ and /bin/lib/firmware/)",
                fname
            );
            return Err(());
        }
    };
    match crate::fs::read(&path) {
        Ok(data) => {
            // struct firmware { size_t size; const u8 *data; }  →  16 byte header
            let buf = kzalloc(16 + data.len(), 0);
            if buf.is_null() {
                return Err(());
            }
            *(buf as *mut u64) = data.len() as u64; // offset 0: size
            *(buf.add(8) as *mut u64) = (buf.add(16)) as u64; // offset 8: data ptr
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf.add(16), data.len());
            crate::mesa_println!("[FW] '{}' loaded {} bytes", fname, data.len());
            Ok(buf)
        }
        Err(_) => {
            crate::mesa_println!("[FW] '{}' read failed", fname);
            Err(())
        }
    }
}

extern "C" fn __shim_request_firmware(fw_ptr: *mut u64, name: *const u8, _dev: *mut u8) -> i32 {
    if fw_ptr.is_null() || name.is_null() {
        return -22;
    }
    // Always NULL-initialize so caller never dereferences garbage on error.
    unsafe { *fw_ptr = 0 };
    let fname = unsafe {
        let mut s = alloc::string::String::new();
        let mut i = 0;
        loop {
            let c = *name.add(i);
            if c == 0 {
                break;
            }
            s.push(c as char);
            i += 1;
            if i > 256 {
                return -36;
            }
        }
        s
    };
    match unsafe { load_firmware_to_buf(&fname) } {
        Ok(buf) => {
            unsafe { *fw_ptr = buf as u64 };
            0
        }
        Err(_) => -2,
    }
}

extern "C" fn __shim_release_firmware(fw: *mut u8) {
    if !fw.is_null() {
        unsafe {
            kfree(fw);
        }
    }
}

// ── Module parameter ops ──────────────────────────────────────
#[repr(C)]
struct KernelParamOps {
    set: u64,
    get: u64,
    free: u64,
}

#[repr(C)]
struct KernelParam {
    name: u64,
    ops: u64,
    perm: u16,
    level: i16,
    arg: u64,
}

extern "C" fn param_set_int(val: *const u8, kp: *const u8) -> i32 {
    if val.is_null() || kp.is_null() {
        return -22;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *mut i32;
    if arg_ptr.is_null() {
        return -22;
    }
    let s = match unsafe { core::ffi::CStr::from_ptr(val as *const i8) }.to_str() {
        Ok(s) => s.trim(),
        Err(_) => return -22,
    };
    let v: i32 = match s.parse() {
        Ok(n) => n,
        Err(_) => return -22,
    };
    unsafe {
        *arg_ptr = v;
    }
    0
}

extern "C" fn param_get_int(buffer: *mut u8, kp: *const u8) -> i32 {
    if buffer.is_null() || kp.is_null() {
        return 0;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *const i32;
    if arg_ptr.is_null() {
        return 0;
    }
    let val = unsafe { *arg_ptr };
    let s = alloc::format!("{}\n", val);
    let bytes = s.as_bytes();
    let len = bytes.len().min(32);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, len);
    }
    len as i32
}

extern "C" fn param_set_uint(val: *const u8, kp: *const u8) -> i32 {
    if val.is_null() || kp.is_null() {
        return -22;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *mut u32;
    if arg_ptr.is_null() {
        return -22;
    }
    let s = match unsafe { core::ffi::CStr::from_ptr(val as *const i8) }.to_str() {
        Ok(s) => s.trim(),
        Err(_) => return -22,
    };
    let v: u32 = match s.parse() {
        Ok(n) => n,
        Err(_) => return -22,
    };
    unsafe {
        *arg_ptr = v;
    }
    0
}

extern "C" fn param_get_uint(buffer: *mut u8, kp: *const u8) -> i32 {
    if buffer.is_null() || kp.is_null() {
        return 0;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *const u32;
    if arg_ptr.is_null() {
        return 0;
    }
    let val = unsafe { *arg_ptr };
    let s = alloc::format!("{}\n", val);
    let bytes = s.as_bytes();
    let len = bytes.len().min(32);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, len);
    }
    len as i32
}

extern "C" fn param_set_long(val: *const u8, kp: *const u8) -> i32 {
    if val.is_null() || kp.is_null() {
        return -22;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *mut i64;
    if arg_ptr.is_null() {
        return -22;
    }
    let s = match unsafe { core::ffi::CStr::from_ptr(val as *const i8) }.to_str() {
        Ok(s) => s.trim(),
        Err(_) => return -22,
    };
    let v: i64 = match s.parse() {
        Ok(n) => n,
        Err(_) => return -22,
    };
    unsafe {
        *arg_ptr = v;
    }
    0
}

extern "C" fn param_get_long(buffer: *mut u8, kp: *const u8) -> i32 {
    if buffer.is_null() || kp.is_null() {
        return 0;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *const i64;
    if arg_ptr.is_null() {
        return 0;
    }
    let val = unsafe { *arg_ptr };
    let s = alloc::format!("{}\n", val);
    let bytes = s.as_bytes();
    let len = bytes.len().min(32);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, len);
    }
    len as i32
}

extern "C" fn param_set_ulong(val: *const u8, kp: *const u8) -> i32 {
    if val.is_null() || kp.is_null() {
        return -22;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *mut u64;
    if arg_ptr.is_null() {
        return -22;
    }
    let s = match unsafe { core::ffi::CStr::from_ptr(val as *const i8) }.to_str() {
        Ok(s) => s.trim(),
        Err(_) => return -22,
    };
    let v: u64 = match s.parse() {
        Ok(n) => n,
        Err(_) => return -22,
    };
    unsafe {
        *arg_ptr = v;
    }
    0
}

extern "C" fn param_get_ulong(buffer: *mut u8, kp: *const u8) -> i32 {
    if buffer.is_null() || kp.is_null() {
        return 0;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *const u64;
    if arg_ptr.is_null() {
        return 0;
    }
    let val = unsafe { *arg_ptr };
    let s = alloc::format!("{}\n", val);
    let bytes = s.as_bytes();
    let len = bytes.len().min(32);
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, len);
    }
    len as i32
}

extern "C" fn param_set_bool(val: *const u8, kp: *const u8) -> i32 {
    if val.is_null() || kp.is_null() {
        return -22;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *mut bool;
    if arg_ptr.is_null() {
        return -22;
    }
    let s = match unsafe { core::ffi::CStr::from_ptr(val as *const i8) }.to_str() {
        Ok(s) => s.trim().to_lowercase(),
        Err(_) => return -22,
    };
    let v = match s.as_str() {
        "1" | "true" | "yes" | "y" | "on" => true,
        "0" | "false" | "no" | "n" | "off" => false,
        _ => return -22,
    };
    unsafe {
        *arg_ptr = v;
    }
    0
}

extern "C" fn param_get_bool(buffer: *mut u8, kp: *const u8) -> i32 {
    if buffer.is_null() || kp.is_null() {
        return 0;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *const bool;
    if arg_ptr.is_null() {
        return 0;
    }
    let val = unsafe { *arg_ptr };
    let s = if val { "Y\n" } else { "N\n" };
    let bytes = s.as_bytes();
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
    }
    bytes.len() as i32
}

extern "C" fn param_set_charp(val: *const u8, kp: *const u8) -> i32 {
    if val.is_null() || kp.is_null() {
        return -22;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *mut *const u8;
    unsafe {
        *arg_ptr = val;
    }
    0
}

extern "C" fn param_get_charp(buffer: *mut u8, kp: *const u8) -> i32 {
    if buffer.is_null() || kp.is_null() {
        return 0;
    }
    let arg_ptr = unsafe { *(kp.add(16) as *const u64) } as *const *const u8;
    if arg_ptr.is_null() {
        return 0;
    }
    let ptr = unsafe { *arg_ptr };
    if ptr.is_null() {
        return 0;
    }
    let len = unsafe { core::ffi::CStr::from_ptr(ptr as *const i8) }
        .to_bytes()
        .len();
    let copy_len = len.min(255);
    unsafe {
        core::ptr::copy_nonoverlapping(ptr, buffer, copy_len);
    }
    if copy_len < 256 {
        unsafe {
            *buffer.add(copy_len) = b'\n';
        }
    }
    (copy_len + 1) as i32
}

lazy_static! {
    static ref PARAM_OPS_INT: KernelParamOps = KernelParamOps {
        set: param_set_int as u64,
        get: param_get_int as u64,
        free: 0,
    };
    static ref PARAM_OPS_CHARP: KernelParamOps = KernelParamOps {
        set: param_set_charp as u64,
        get: param_get_charp as u64,
        free: 0,
    };
    static ref PARAM_OPS_BOOL: KernelParamOps = KernelParamOps {
        set: param_set_bool as u64,
        get: param_get_bool as u64,
        free: 0,
    };
    static ref PARAM_OPS_UINT: KernelParamOps = KernelParamOps {
        set: param_set_uint as u64,
        get: param_get_uint as u64,
        free: 0,
    };
    static ref PARAM_OPS_LONG: KernelParamOps = KernelParamOps {
        set: param_set_long as u64,
        get: param_get_long as u64,
        free: 0,
    };
    static ref PARAM_OPS_ULONG: KernelParamOps = KernelParamOps {
        set: param_set_ulong as u64,
        get: param_get_ulong as u64,
        free: 0,
    };
}

extern "C" fn __shim_module_layout() {}
extern "C" fn __shim_param_ops_int() -> u64 {
    &*PARAM_OPS_INT as *const KernelParamOps as u64
}
extern "C" fn __shim_param_ops_charp() -> u64 {
    &*PARAM_OPS_CHARP as *const KernelParamOps as u64
}
extern "C" fn __shim_param_ops_bool() -> u64 {
    &*PARAM_OPS_BOOL as *const KernelParamOps as u64
}
extern "C" fn __shim_param_ops_uint() -> u64 {
    &*PARAM_OPS_UINT as *const KernelParamOps as u64
}
extern "C" fn __shim_param_ops_long() -> u64 {
    &*PARAM_OPS_LONG as *const KernelParamOps as u64
}
extern "C" fn __shim_param_ops_ulong() -> u64 {
    &*PARAM_OPS_ULONG as *const KernelParamOps as u64
}

extern "C" fn __shim_try_module_get(name: *const u8) -> i32 {
    if name.is_null() {
        return 0;
    }
    let s = match unsafe { core::ffi::CStr::from_ptr(name as *const i8) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    if crate::shim::loader::try_module_get(s) {
        1
    } else {
        0
    }
}

extern "C" fn __shim_module_put(name: *const u8) {
    if name.is_null() {
        return;
    }
    let s = match unsafe { core::ffi::CStr::from_ptr(name as *const i8) }.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    crate::shim::loader::module_put(s);
}

pub fn find_symbol(name: &str) -> Option<usize> {
    KERNEL_SYMBOLS
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.addr)
}

pub fn lookup_symbol(name: &str) -> Option<*mut u8> {
    find_symbol(name).map(|addr| addr as *mut u8)
}

// ── mac80211/cfg80211 Shim ──────────────────────────────────────
const IEEE80211_HW_SIZE: usize = 192;

static HW_OPS: spin::Mutex<BTreeMap<u64, u64>> = spin::Mutex::new(BTreeMap::new());
static WIPHY_TO_HW: spin::Mutex<BTreeMap<u64, u64>> = spin::Mutex::new(BTreeMap::new());

extern "C" fn __shim_ieee80211_alloc_hw(sizeof_priv: i32, ops: *const u8) -> *mut u8 {
    let total = IEEE80211_HW_SIZE + sizeof_priv.max(0) as usize;
    let hw = unsafe { kzalloc(total, 0) };
    if hw.is_null() {
        return core::ptr::null_mut();
    }
    unsafe {
        *(hw.add(80) as *mut *mut u8) = hw.add(IEEE80211_HW_SIZE);
    }
    let wiphy = unsafe { kzalloc(4096, 0) };
    if !wiphy.is_null() {
        unsafe {
            *(hw.add(64) as *mut *mut u8) = wiphy;
        }
        WIPHY_TO_HW.lock().insert(wiphy as u64, hw as u64);
    }
    if !ops.is_null() {
        HW_OPS.lock().insert(hw as u64, ops as u64);
    }
    crate::mesa_println!("[MAC80211] alloc_hw: hw={:p} priv_size={}", hw, sizeof_priv);
    hw
}

extern "C" fn __shim_ieee80211_register_hw(hw: *mut u8) -> i32 {
    if hw.is_null() {
        return -22;
    }
    crate::mesa_println!("[MAC80211] register_hw: {:p}", hw);
    0
}

extern "C" fn __shim_ieee80211_unregister_hw(hw: *mut u8) {
    if !hw.is_null() {
        crate::mesa_println!("[MAC80211] unregister_hw: {:p}", hw);
    }
}

extern "C" fn __shim_ieee80211_free_hw(hw: *mut u8) {
    if hw.is_null() {
        return;
    }
    let wiphy = unsafe { *(hw.add(64) as *mut *mut u8) };
    if !wiphy.is_null() {
        WIPHY_TO_HW.lock().remove(&(wiphy as u64));
        unsafe {
            kfree(wiphy);
        }
    }
    HW_OPS.lock().remove(&(hw as u64));
    unsafe {
        kfree(hw);
    }
    crate::mesa_println!("[MAC80211] free_hw: {:p}", hw);
}

extern "C" fn __shim_ieee80211_stop_queues(hw: *mut u8) {
    if !hw.is_null() {
        crate::mesa_println!("[MAC80211] stop_queues: {:p}", hw);
    }
}

extern "C" fn __shim_ieee80211_wake_queues(hw: *mut u8) {
    if !hw.is_null() {
        crate::mesa_println!("[MAC80211] wake_queues: {:p}", hw);
    }
}

extern "C" fn __shim_ieee80211_stop_queue(hw: *mut u8, queue: u32) {
    if !hw.is_null() {
        crate::mesa_println!("[MAC80211] stop_queue: {:p} queue={}", hw, queue);
    }
}

extern "C" fn __shim_ieee80211_wake_queue(hw: *mut u8, queue: u32) {
    if !hw.is_null() {
        crate::mesa_println!("[MAC80211] wake_queue: {:p} queue={}", hw, queue);
    }
}

extern "C" fn __shim_ieee80211_tx_status_irqsafe(hw: *mut u8, skb: *mut u8) {
    if !skb.is_null() {
        crate::mesa_println!("[MAC80211] tx_status_irqsafe: hw={:p} skb={:p}", hw, skb);
        unsafe {
            kfree(skb);
        }
    }
}

extern "C" fn __shim_ieee80211_rx_napi(hw: *mut u8, sta: *mut u8, skb: *mut u8, napi: *mut u8) {
    if !skb.is_null() {
        let len = skb_rd32(skb, 0x20);
        crate::mesa_println!("[MAC80211] rx_napi: hw={:p} skb={:p} len={}", hw, skb, len);
        if len > 0 {
            let data = skb_rd64(skb, 0x08);
            if !data.is_null() {
                let packet = unsafe { core::slice::from_raw_parts(data, len as usize) };
                if let Some(mac) = crate::drivers::net::virtio_net::get_mac() {
                    let _ = crate::drivers::net::virtio_net::send_packet(packet);
                } else if let Some(mac) = crate::drivers::net::rtl8139::get_mac() {
                    let _ = crate::drivers::net::rtl8139::send_packet(packet);
                }
            }
        }
        unsafe {
            kfree(skb);
        }
    }
}

extern "C" fn __shim_ieee80211_rx_irqsafe(hw: *mut u8, skb: *mut u8) {
    if !skb.is_null() {
        let len = skb_rd32(skb, 0x20);
        crate::mesa_println!(
            "[MAC80211] rx_irqsafe: hw={:p} skb={:p} len={}",
            hw,
            skb,
            len
        );
        unsafe {
            kfree(skb);
        }
    }
}

extern "C" fn __shim_ieee80211_find_sta(vif: *mut u8, addr: *const u8) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_find_sta_by_ifaddr(
    hw: *mut u8,
    addr1: *const u8,
    addr2: *const u8,
) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_iterate_stations_atomic(hw: *mut u8, iterator: u64, data: *mut u8) {}

extern "C" fn __shim_ieee80211_iterate_active_interfaces_atomic(
    hw: *mut u8,
    iterator: u64,
    data: *mut u8,
) {
}

extern "C" fn __shim_ieee80211_beacon_get_tim(
    hw: *mut u8,
    vif: *mut u8,
    tim_offset: *mut u32,
    tim_length: *mut u32,
) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_scan_completed(hw: *mut u8, info: *const u8) {
    crate::mesa_println!("[MAC80211] scan_completed: hw={:p}", hw);
}

extern "C" fn __shim_ieee80211_connection_loss(vif: *mut u8) {}

extern "C" fn __shim_ieee80211_queue_work(hw: *mut u8, work: *mut u8) {
    if !work.is_null() {
        let func = unsafe { *(work as *const usize) };
        if func != 0 {
            SHIM_WORKQUEUE.lock().push(ShimWork { func });
        }
    }
}

extern "C" fn __shim_ieee80211_queue_delayed_work(hw: *mut u8, dwork: *mut u8, delay: u32) {
    if !dwork.is_null() {
        let func = unsafe { *(dwork as *const usize) };
        if func != 0 {
            SHIM_WORKQUEUE.lock().push(ShimWork { func });
        }
    }
}

extern "C" fn __shim_ieee80211_channel_to_frequency(chan: i32, band: u32) -> i32 {
    match band {
        0 => match chan {
            1 => 2412,
            2 => 2417,
            3 => 2422,
            4 => 2427,
            5 => 2432,
            6 => 2437,
            7 => 2442,
            8 => 2447,
            9 => 2452,
            10 => 2457,
            11 => 2462,
            12 => 2467,
            13 => 2472,
            14 => 2484,
            _ => 2412,
        },
        1 => {
            if chan >= 1 && chan <= 200 {
                5000 + (chan as i32) * 5
            } else {
                5000
            }
        }
        _ => 2412,
    }
}

extern "C" fn __shim_ieee80211_free_txskb(hw: *mut u8, skb: *mut u8) {
    if !skb.is_null() {
        unsafe {
            kfree(skb);
        }
    }
}

extern "C" fn __shim_ieee80211_tx_dequeue(hw: *mut u8, txq: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_tx_info_clear_status(status: *mut u8) {}

extern "C" fn __shim_ieee80211_txq_get_depth(txq: *mut u8, frame_cnt: *mut u32) {
    if !frame_cnt.is_null() {
        unsafe {
            *frame_cnt = 0;
        }
    }
}

extern "C" fn __shim_ieee80211_start_tx_ba_session(sta: *mut u8, tid: u16, timeout: u16) -> i32 {
    0
}

extern "C" fn __shim_ieee80211_stop_tx_ba_cb_irqsafe(vif: *mut u8, addr: *const u8, tid: u16) {}

extern "C" fn __shim_ieee80211_purge_tx_queue(hw: *mut u8, txq: *mut u8) {}

extern "C" fn __shim_ieee80211_restart_hw(hw: *mut u8) {
    crate::mesa_println!("[MAC80211] restart_hw: {:p}", hw);
}

extern "C" fn __shim_ieee80211_request_smps(vif: *mut u8, link_id: u32, smps_mode: u32) -> i32 {
    0
}

extern "C" fn __shim_ieee80211_cqm_rssi_notify(vif: *mut u8, event: u32, sig: i32, gfp: u32) {}

extern "C" fn __shim_ieee80211_report_wowlan_wakeup(vif: *mut u8, wakeup: *mut u8, gfp: u32) {}

extern "C" fn __shim_ieee80211_create_tpt_led_trigger(
    hw: *mut u8,
    flags: u32,
    blink_set: u64,
) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_pspoll_get(hw: *mut u8, vif: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_nullfunc_get(
    hw: *mut u8,
    vif: *mut u8,
    link_id: i32,
    qos: bool,
) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_proberesp_get(hw: *mut u8, vif: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_probereq_get(
    hw: *mut u8,
    addr: *const u8,
    ssid: *const u8,
    ssid_len: usize,
    tailroom: usize,
) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_ieee80211_vif_type_p2p(vif: *mut u8) -> u32 {
    if vif.is_null() {
        return 0;
    }
    unsafe { *(vif as *const u32) }
}

extern "C" fn __shim_netif_napi_add(dev: *mut u8, napi: *mut u8, poll: u64, weight: i32) {
    if !napi.is_null() {
        crate::mesa_println!(
            "[NAPI] netif_napi_add: napi={:p} poll={:#x} weight={}",
            napi,
            poll,
            weight
        );
    }
}

extern "C" fn __shim_napi_enable(napi: *mut u8) {}

extern "C" fn __shim_napi_disable(napi: *mut u8) {}

extern "C" fn __shim_cfg80211_calculate_bitrate(rate: *const u8) -> u32 {
    0
}

extern "C" fn __shim_cfg80211_ssid_eq(a: *const u8, b: *const u8) -> bool {
    false
}

extern "C" fn __shim_cfg80211_get_ies_channel_number(
    ie: *const u8,
    ielen: usize,
    band: u32,
) -> i32 {
    0
}

extern "C" fn __shim_wiphy_to_ieee80211_hw(wiphy: *mut u8) -> *mut u8 {
    if wiphy.is_null() {
        return core::ptr::null_mut();
    }
    let map = WIPHY_TO_HW.lock();
    map.get(&(wiphy as u64))
        .map(|&hw| hw as *mut u8)
        .unwrap_or(core::ptr::null_mut())
}

// ── New shim stubs ────────────────────────────────────────────────

extern "C" fn __shim_netif_napi_del(_napi: *mut u8) {}

extern "C" fn __shim_napi_schedule(_napi: *mut u8) {}

extern "C" fn __shim_napi_synchronize(_napi: *mut u8) {}

extern "C" fn __shim_napi_complete_done(_napi: *mut u8, _work_done: i32) -> bool {
    true
}

extern "C" fn __shim_pci_iomap(dev: *mut u8, bar: i32, _maxlen: u64) -> *mut u8 {
    if dev.is_null() {
        return core::ptr::null_mut();
    }
    let (bus, device, function) = pci_dev_to_bdf(dev);
    match crate::pci::pci_read_bar(bus, device, function, bar as u8) {
        Some((start, size)) if start != 0 => {
            let map_size = if _maxlen != 0 && _maxlen < size {
                _maxlen
            } else {
                size
            };
            match crate::memory::vmm::map_mmio(start, map_size) {
                Ok(virt) => {
                    crate::mesa_println!(
                        "[PCI] iomap bar{}: phys={:#x} size={:#x} virt={:#x}",
                        bar,
                        start,
                        map_size,
                        virt
                    );
                    virt as *mut u8
                }
                Err(e) => {
                    crate::mesa_println!("[PCI] iomap bar{}: map_mmio failed: {}", bar, e);
                    core::ptr::null_mut()
                }
            }
        }
        _ => {
            crate::mesa_println!("[PCI] iomap bar{}: FAILED (no BAR)", bar);
            core::ptr::null_mut()
        }
    }
}

extern "C" fn __shim_pci_iounmap(_dev: *mut u8, _addr: *mut u8) {}

extern "C" fn __shim_pci_alloc_irq_vectors(
    _dev: *mut u8,
    _min_vecs: u32,
    _max_vecs: u32,
    _flags: u32,
) -> i32 {
    0
}

extern "C" fn __shim_pci_free_irq_vectors(_dev: *mut u8) {}

extern "C" fn __shim_pcie_capability_read_word(_dev: *mut u8, _pos: i32, _val: *mut u16) -> i32 {
    unsafe {
        *_val = 0;
    }
    0
}

extern "C" fn __shim_pcie_capability_set_word(_dev: *mut u8, _pos: i32, _set: u16) -> i32 {
    0
}

extern "C" fn __shim_pci_upstream_bridge(_dev: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_pci_set_power_state(_dev: *mut u8, _state: i32) -> i32 {
    0
}

extern "C" fn __shim_pci_enable_wake(_dev: *mut u8, _state: i32, _enable: bool) -> i32 {
    0
}

extern "C" fn __shim_devm_request_threaded_irq(
    _dev: *mut u8,
    _irq: u32,
    _handler: *mut u8,
    _thread_fn: *mut u8,
    _flags: u64,
    _name: *const u8,
    _dev_id: *mut u8,
) -> i32 {
    0
}

extern "C" fn __shim_devm_free_irq(_dev: *mut u8, _irq: u32, _dev_id: *mut u8) {}

extern "C" fn __shim_alloc_netdev_dummy(_sizeof_priv: i32) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_skb_copy(_skb: *const u8, _gfp: u32) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_skb_pull(_skb: *mut u8, _len: u32) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_complete_all(_c: *mut u8) {}

extern "C" fn __shim_request_firmware_nowait(
    _mod: *mut u8,
    _uevent: bool,
    _name: *const u8,
    _dev: *mut u8,
    _gfp: u32,
    _ctx: *mut u8,
    _cb: *mut u8,
) -> i32 {
    if _name.is_null() || _cb.is_null() {
        return -22;
    }
    let fname = unsafe {
        let mut s = alloc::string::String::new();
        let mut i = 0;
        loop {
            let c = *_name.add(i);
            if c == 0 {
                break;
            }
            s.push(c as char);
            i += 1;
            if i > 256 {
                return -36;
            }
        }
        s
    };
    let fw_buf = unsafe { load_firmware_to_buf(&fname).unwrap_or(core::ptr::null_mut()) };
    let cb: extern "C" fn(*const u8, *mut u8) = unsafe { core::mem::transmute(_cb) };
    cb(fw_buf as *const u8, _ctx);
    0
}

extern "C" fn __shim_alloc_skb(size: u32) -> *mut u8 {
    let p = unsafe { kmalloc(size as usize, 0) };
    if !p.is_null() {
        unsafe { core::ptr::write_bytes(p, 0, size as usize) };
    }
    p
}

extern "C" fn __shim_dev_kfree_skb_any(skb: *mut u8) {
    if !skb.is_null() {
        unsafe { kfree(skb) }
    }
}

extern "C" fn __shim_skb_dequeue(_list: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_skb_put_data(_skb: *mut u8, _data: *const u8, _len: u32) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_skb_queue_purge(_list: *mut u8) {}

extern "C" fn __shim___skb_queue_tail(_list: *mut u8, _skb: *mut u8) {}

extern "C" fn __shim_skb_queue_tail(_list: *mut u8, _skb: *mut u8) {}

extern "C" fn __shim___skb_unlink(_skb: *mut u8, _list: *mut u8) {}

extern "C" fn __shim_skb_unlink(_skb: *mut u8, _list: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_alloc_workqueue(_name: *const u8, _flags: u32, _max_active: i32) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_destroy_workqueue(_wq: *mut u8) {}

extern "C" fn __shim_timer_delete_sync(_t: *mut u8) -> bool {
    true
}

extern "C" fn __shim_devm_kmemdup(
    _dev: *mut u8,
    _src: *const u8,
    _len: usize,
    _gfp: u32,
) -> *mut u8 {
    core::ptr::null_mut()
}

extern "C" fn __shim_regulatory_hint(_wiphy: *mut u8, _alpha2: *const u8) -> i32 {
    0
}

extern "C" fn __shim_get_random_mask_addr(_addr: *mut u8, _mask: *const u8, _addr2: *const u8) {}

extern "C" fn __shim_ieee80211_emulate_add_chanctx(_hw: *mut u8, _ctx: *mut u8) -> i32 {
    0
}

extern "C" fn __shim_ieee80211_emulate_remove_chanctx(_hw: *mut u8, _ctx: *mut u8) {}

extern "C" fn __shim_ieee80211_emulate_change_chanctx(_hw: *mut u8, _ctx: *mut u8, _changed: u32) {}

extern "C" fn __shim_ieee80211_emulate_switch_vif_chanctx(
    _hw: *mut u8,
    _vifs: *mut u8,
    _n_vifs: i32,
    _old_ctx: *mut u8,
    _new_ctx: *mut u8,
) -> i32 {
    0
}

// jiffies global variable
#[no_mangle]
static __shim_jiffies: u64 = 0;

#[no_mangle]
pub extern "C" fn __popcountdi2(x: u64) -> i32 {
    x.count_ones() as i32
}

#[no_mangle]
pub extern "C" fn __shim___fentry__() {}

#[no_mangle]
pub extern "C" fn __shim___x86_return_thunk() {}

#[no_mangle]
pub extern "C" fn __shim___ubsan_handle_out_of_bounds(_data: *mut u8, _index: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_is_primary_hcd(_hcd: *mut u8) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_poll_rh_status(_hcd: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_resume_root_hub(_hcd: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_map_urb_for_dma(
    _hcd: *mut u8,
    _urb: *mut u8,
    _mem_flags: u32,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_unmap_urb_for_dma(_hcd: *mut u8, _urb: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_sg_pcopy_from_buffer(
    _sgl: *mut u8,
    _nents: u32,
    _buf: *mut u8,
    _buflen: usize,
    _skip: usize,
) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn __shim_sg_pcopy_to_buffer(
    _sgl: *mut u8,
    _nents: u32,
    _buf: *mut u8,
    _buflen: usize,
    _skip: usize,
) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn __shim_is_vmalloc_addr(_addr: *const u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_dev_driver_string(_dev: *const u8) -> *const u8 {
    b"xhci_hcd\0".as_ptr()
}

#[no_mangle]
pub extern "C" fn __shim___sw_hweight32(w: u32) -> u32 {
    w.count_ones()
}

#[no_mangle]
pub static __shim_page_offset_base: u64 = 0xffff800000000000;

#[no_mangle]
pub static __shim_vmemmap_base: u64 = 0xffffea0000000000;

#[no_mangle]
pub static __shim_phys_base: u64 = 0;

// === Batch 2: XHCI additional stubs ===

#[no_mangle]
pub extern "C" fn __shim_delayed_work_timer_fn(_timer: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_timer_init_key(
    _timer: *mut u8,
    _key: *mut u8,
    _name: *const u8,
    _flags: u32,
) {
}

#[no_mangle]
pub extern "C" fn __shim_init_swait_queue_head(_q: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_dmi_get_system_info(_field: i32) -> *const u8 {
    b"MesaOS\0".as_ptr()
}

#[no_mangle]
pub extern "C" fn __shim_strstr(haystack: *const u8, needle: *const u8) -> *const u8 {
    if haystack.is_null() || needle.is_null() {
        return core::ptr::null();
    }
    unsafe {
        let h = core::ffi::CStr::from_ptr(haystack as *const i8).to_bytes();
        let n = core::ffi::CStr::from_ptr(needle as *const i8).to_bytes();
        if n.is_empty() {
            return haystack;
        }
        for i in 0..h.len() {
            if h[i..].starts_with(n) {
                return haystack.add(i);
            }
        }
    }
    core::ptr::null()
}

// CPU topology globals
#[no_mangle]
pub static __shim_cpu_number: u32 = 0;

#[no_mangle]
pub static __shim_cpu_online_mask: [u8; 16] = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

#[no_mangle]
pub static __shim_preempt_count: i32 = 0;

#[no_mangle]
pub extern "C" fn __shim_preempt_schedule_notrace() {}

#[no_mangle]
pub extern "C" fn __shim_schedule_timeout_uninterruptible(_timeout: i64) -> i64 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_check_unlink_urb(
    _hcd: *mut u8,
    _urb: *mut u8,
    _status: i32,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_unlink_urb_from_ep(_hcd: *mut u8, _urb: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_giveback_urb(_hcd: *mut u8, _urb: *mut u8, _status: i32) {}

#[no_mangle]
pub extern "C" fn __shim_usleep_range_state(_min: u64, _max: u64, _state: u32) {}

#[no_mangle]
pub extern "C" fn __shim_usb_asmedia_modifyflowcontrol(_pdev: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_usb_disable_xhci_ports(_pdev: *mut u8) {}

// === Batch 3: xHCI additional stubs ===

#[no_mangle]
pub extern "C" fn __shim___x86_indirect_thunk_rX() {}

#[no_mangle]
pub extern "C" fn __shim_iommu_get_domain_for_dev(_dev: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_dma_set_mask(_dev: *mut u8, _mask: u64) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_dma_set_coherent_mask(_dev: *mut u8, _mask: u64) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_amd_dev_put(_pdev: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_usb_root_hub_lost_power(_rhdev: *mut u8) {}

#[no_mangle]
pub static __shim_stack_chk_guard: u64 = 0x5f9cff773d2a30f8;

#[no_mangle]
pub extern "C" fn __shim_stack_chk_fail() {
    crate::serial_println!("[SHIM] Stack smashing detected!");
}

#[no_mangle]
pub extern "C" fn __shim_ktime_get() -> i64 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_might_resched() {}

#[no_mangle]
pub extern "C" fn __shim_dma_pool_create_node(
    _name: *const u8,
    _dev: *mut u8,
    _size: usize,
    _align: usize,
    _boundary: usize,
    _node: i32,
) -> *mut u8 {
    1usize as *mut u8
}

#[no_mangle]
pub extern "C" fn __shim_dma_pool_destroy(_pool: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_dma_pool_alloc(
    _pool: *mut u8,
    flags: u32,
    dma_handle: *mut u64,
) -> *mut u8 {
    unsafe {
        let ptr = crate::linux::kmalloc(4096, flags);
        if !ptr.is_null() && !dma_handle.is_null() {
            *dma_handle = ptr as u64;
        }
        ptr
    }
}

#[no_mangle]
pub extern "C" fn __shim_dma_pool_free(_pool: *mut u8, vaddr: *mut u8, _dma: u64) {
    if !vaddr.is_null() {
        unsafe {
            crate::linux::kfree(vaddr);
        }
    }
}

#[no_mangle]
pub static __shim_random_kmalloc_seed: u64 = 0xdeadbeefcafe1234;

#[no_mangle]
pub static __shim_kmalloc_caches: [u64; 4] = [0; 4];

#[no_mangle]
pub static __shim_system_percpu_wq: u64 = 0;

#[no_mangle]
pub extern "C" fn __shim_radix_tree_lookup(_root: *mut u8, _index: u64) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_radix_tree_maybe_preload(_flags: u32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_radix_tree_insert(_root: *mut u8, _index: u64, _item: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_radix_tree_delete(_root: *mut u8, _index: u64) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_cancel_delayed_work_sync(_work: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_platform_device_alloc(_name: *const u8, _id: i32) -> *mut u8 {
    unsafe { crate::linux::kmalloc(512, 0x14) }
}

#[no_mangle]
pub extern "C" fn __shim_platform_device_add_resources(
    _pdev: *mut u8,
    _res: *const u8,
    _num: u32,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_platform_device_add(_pdev: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_platform_device_unregister(_pdev: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_platform_device_put(_pdev: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_devm_add_action(_dev: *mut u8, _action: u64, _data: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_device_create_managed_software_node(
    _dev: *mut u8,
    _props: *const u8,
    _parent: *const u8,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_ratelimit(_rs: *mut u8) -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_link_urb_to_ep(_hcd: *mut u8, _urb: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_msecs_to_jiffies(m: u32) -> u64 {
    m as u64
}

#[no_mangle]
pub extern "C" fn __shim_mod_delayed_work_on(
    _cpu: i32,
    _wq: *mut u8,
    _work: *mut u8,
    _delay: u64,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_amd_quirk_pll_enable() {}

// === Batch 4: XHCI final USB + trace symbols ===

#[no_mangle]
pub extern "C" fn __shim_usb_hub_clear_tt_buffer(
    _hdev: *mut u8,
    _devinfo: u16,
    _tt: *mut u8,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_cancel_delayed_work(_work: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_hc_died(_hcd: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_wait_for_completion_timeout(_x: *mut u8, _timeout: u64) -> u64 {
    1
}

#[no_mangle]
pub extern "C" fn __shim_usb_wakeup_notification(_hdev: *mut u8, _portnum: u32, _is_wakeup: i32) {}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_start_port_resume(_bus: *mut u8, _portnum: i32) {}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_end_port_resume(_bus: *mut u8, _portnum: i32) {}

#[no_mangle]
pub extern "C" fn __shim_fortify_panic(_func: *const u8) {
    crate::serial_println!("[SHIM] fortify_panic called!");
}

#[no_mangle]
pub extern "C" fn __shim_usb_amd_quirk_pll_disable() {}

#[no_mangle]
pub extern "C" fn __shim_usb_acpi_power_manageable(_hdev: *mut u8, _index: i32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_acpi_set_power_state(
    _hdev: *mut u8,
    _index: i32,
    _enable: i32,
) -> i32 {
    0
}

#[no_mangle]
pub static __shim_pci_bus_type: [u8; 64] = [0; 64];

#[no_mangle]
pub extern "C" fn __shim_pm_runtime_allow(_dev: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_pm_runtime_forbid(_dev: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_usb_amd_pt_check_port(_rhdev: *mut u8, _port: i32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_vsnprintf(buf: *mut u8, size: usize, fmt: *const u8, ap: u64) -> i32 {
    // Minimal fallback - just copy fmt string safely
    if buf.is_null() || size == 0 {
        return 0;
    }
    unsafe {
        let n = if size > 1 { size - 1 } else { 0 };
        let mut i = 0usize;
        while i < n {
            let c = *fmt.add(i);
            if c == 0 {
                break;
            }
            *buf.add(i) = c;
            i += 1;
        }
        *buf.add(i) = 0;
        i as i32
    }
}

#[no_mangle]
pub static __shim_this_cpu_off: u64 = 0;

#[no_mangle]
pub extern "C" fn __shim_perf_trace_buf_alloc(
    _size: i32,
    _entry: *mut *mut u8,
    _rctx: *mut i32,
) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_perf_trace_run_bpf_submit(
    _raw_data: *mut u8,
    _size: i32,
    _rctx: i32,
    _call: *mut u8,
    _count: u64,
    _regs: *mut u8,
    _head: *mut u8,
    _rctxp: *mut u8,
) {
}

#[no_mangle]
pub extern "C" fn __shim_trace_event_buffer_reserve(
    _fbuffer: *mut u8,
    _ctx: *mut u8,
    _len: usize,
) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_trace_event_buffer_commit(_fbuffer: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_trace_trigger_soft_disabled(_file: *mut u8) -> i32 {
    1
}

// === Batch 5 stubs: Trace output, sysfs, kstrtou*, bpf, PM, TTY, idr, kfifo ===

#[no_mangle]
pub extern "C" fn __shim_trace_raw_output_prep(_iter: *mut u8, _fmt: *mut u8, _len: usize) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_trace_event_printf(_iter: *mut u8, _fbuffer: *mut u8, _fmt: *const u8) {}

#[no_mangle]
pub extern "C" fn __shim_trace_handle_return(_s: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_trace_print_symbols_seq(
    _s: *mut u8,
    _val: u64,
    _symbols: *mut u8,
    _size: u32,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_trace_seq_acquire(_s: *mut u8, _len: usize) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_bpf_trace_run1(_call: *mut u8, _a1: u64) {}

#[no_mangle]
pub extern "C" fn __shim_bpf_trace_run2(_call: *mut u8, _a1: u64, _a2: u64) {}

#[no_mangle]
pub extern "C" fn __shim_bpf_trace_run3(_call: *mut u8, _a1: u64, _a2: u64, _a3: u64) {}

#[no_mangle]
pub extern "C" fn __shim_kstrtouint(_s: *const u8, _base: u32, _res: *mut u32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_sysfs_emit(_buf: *mut u8, _fmt: *const u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_kstrtou8(_s: *const u8, _base: u32, _res: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_kstrtou16(_s: *const u8, _base: u32, _res: *mut u16) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_strcspn(_s: *const u8, _reject: *const u8) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn __shim_utf8s_to_utf16s(
    _s: *const u8,
    _len: usize,
    _endian: u32,
    _dst: *mut u16,
    _maxlen: usize,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim___pm_runtime_idle(_dev: *mut u8, _rpmflags: i32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim___pm_runtime_resume(_dev: *mut u8, _rpmflags: i32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_sysfs_streq(_s1: *const u8, _s2: *const u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_sysfs_create_groups(_kobj: *mut u8, _groups: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_sysfs_remove_groups(_kobj: *mut u8, _groups: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim___tasklet_schedule(_t: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_tasklet_setup(_t: *mut u8, _func: *mut u8, _data: u64) {}

#[no_mangle]
pub extern "C" fn __shim_tty_port_close(_port: *mut u8, _tty: *mut u8, _filp: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_tty_port_open(_port: *mut u8, _tty: *mut u8, _filp: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_idr_find(_idr: *mut u8, _id: u32) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_tty_port_install(_port: *mut u8, _driver: *mut u8, _tty: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim___tty_insert_flip_string_flags(
    _port: *mut u8,
    _chars: *mut u8,
    _flags: *mut u8,
    _size: usize,
) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn __shim_tty_flip_buffer_push(_port: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim___kfifo_out(_fifo: *mut u8, _buf: *mut u8, _len: usize) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn __shim_tty_wakeup(_tty: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim___kfifo_in(_fifo: *mut u8, _buf: *mut u8, _len: usize) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn __shim___tty_port_tty_hangup(_port: *mut u8, _check_clocal: i32) {}

#[no_mangle]
pub extern "C" fn __shim_tty_unregister_device(_driver: *mut u8, _index: u32) {}

#[no_mangle]
pub extern "C" fn __shim_tty_port_destroy(_port: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_idr_remove(_idr: *mut u8, _id: u32) {}

#[no_mangle]
pub extern "C" fn __shim___kfifo_free(_fifo: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_tty_port_init(_port: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_idr_alloc(
    _idr: *mut u8,
    _ptr: *mut u8,
    _start: u32,
    _end: u32,
    _gfp: u32,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim___kfifo_alloc(
    _fifo: *mut u8,
    _size: usize,
    _esize: usize,
    _gfp: u32,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_tty_port_register_device(
    _driver: *mut u8,
    _index: u32,
    _device: *mut u8,
    _port: *mut u8,
) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim___tty_alloc_driver(_lines: u32, _flags: u64) -> *mut u8 {
    crate::printk!(
        "[SHIM] __tty_alloc_driver(lines={}, flags={:#x}) - returning ERR_PTR(-ENOMEM)",
        _lines,
        _flags
    );
    // Return ERR_PTR(-ENOMEM) so the module handles failure gracefully instead of
    // treating NULL as a valid pointer and crashing.
    (-12isize) as *mut u8
}

// === Batch 6 stubs: TTY, debugfs, seq, uaccess ===

#[no_mangle]
pub static __shim_tty_std_termios: [u8; 64] = [0; 64];

#[no_mangle]
pub extern "C" fn __shim_tty_register_driver(_driver: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_tty_driver_kref_put(_driver: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_idr_destroy(_idr: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_tty_unregister_driver(_driver: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_debugfs_get_aux(_dentry: *mut u8) -> u64 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_single_open(_inode: *mut u8, _file: *mut u8, _seq_show: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_seq_printf(_m: *mut u8, _fmt: *const u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_kstrtou16_from_user(_s: *mut u8, _base: u32, _res: *mut u16) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim___check_object_size(_ptr: *mut u8, _n: usize, _write: i32) {}

#[no_mangle]
pub extern "C" fn __shim__copy_from_user(_to: *mut u8, _from: *mut u8, _n: usize) -> usize {
    0
}

#[no_mangle]
pub extern "C" fn __shim_debugfs_create_regset32(
    _name: *const u8,
    _mode: u16,
    _parent: *mut u8,
    _regset: *mut u8,
) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_debugfs_create_dir(_name: *const u8, _parent: *mut u8) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_debugfs_create_file_full(
    _name: *const u8,
    _mode: u16,
    _parent: *mut u8,
    _data: *mut u8,
    _fops: *mut u8,
) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_debugfs_remove(_dentry: *mut u8) {}

// === Batch 7 stubs: xhci-hcd final + xhci-pci ===

#[no_mangle]
pub extern "C" fn __shim_scnprintf(buf: *mut u8, size: usize, fmt: *const u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim___kvmalloc_node_noprof(_size: usize, _flags: u32, _node: i32) -> *mut u8 {
    core::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn __shim_dma_get_sgtable_attrs(
    _dev: *mut u8,
    _table: *mut u8,
    _cpu_addr: *mut u8,
    _dma_handle: u64,
    _size: usize,
    _attrs: u64,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_sg_free_table(_table: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim_sg_alloc_table_from_pages_segment(
    _table: *mut u8,
    _pages: *mut u8,
    _n_pages: u32,
    _offset: u32,
    _size: usize,
    _seg_size: u32,
    _gfp: u32,
) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_kvfree(_ptr: *mut u8) {
    unsafe { kfree(_ptr) }
}

#[no_mangle]
pub extern "C" fn __shim___ubsan_handle_shift_out_of_bounds(_data: *mut u8, _lhs: u64, _rhs: u64) {}

#[no_mangle]
pub extern "C" fn __shim___ubsan_handle_load_invalid_value(_data: *mut u8, _val: u64) {}

#[no_mangle]
pub extern "C" fn __shim_usb_disabled() -> i32 {
    0
}

#[no_mangle]
pub static __shim_usb_debug_root: u64 = 0;

#[no_mangle]
pub extern "C" fn __shim_seq_lseek(_file: *mut u8, _offset: i64, _whence: i32) -> i64 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_seq_read(
    _file: *mut u8,
    _buf: *mut u8,
    _size: usize,
    _ppos: *mut i64,
) -> i64 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_single_release(_inode: *mut u8, _file: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub static __shim_param_ops_ullong: [u8; 64] = [0; 64];

#[no_mangle]
pub extern "C" fn __shim_validate_usercopy_range(_s: *mut u8, _n: usize, _type: u32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_trace_event_reg(_file: *mut u8, _call: *mut u8, _reg: i32) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_trace_event_raw_init(_file: *mut u8, _call: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn __shim_pci_dev_get(_dev: *mut u8) -> *mut u8 {
    _dev
}

#[no_mangle]
pub extern "C" fn __shim_firmware_request_nowarn(
    fw_ptr: *mut *mut u8,
    _name: *const u8,
    _dev: *mut u8,
) -> i32 {
    // Return -ENOENT: firmware file not available.
    // We MUST set *fw_ptr to NULL so the caller doesn't dereference garbage.
    if !fw_ptr.is_null() {
        unsafe {
            *fw_ptr = core::ptr::null_mut();
        }
    }
    -2
}

#[no_mangle]
pub extern "C" fn __shim_pci_dev_put(_dev: *mut u8) {}

#[no_mangle]
pub extern "C" fn __shim___pci_register_driver(
    drv: *mut u8,
    _owner: *mut u8,
    mod_name: *const u8,
) -> i32 {
    if drv.is_null() {
        return -22;
    }
    unsafe {
        let name_ptr = *(drv as *mut *mut u8);
        // If the driver has no explicit name, fall back to mod_name
        let effective_name = if name_ptr.is_null() {
            mod_name
        } else {
            name_ptr
        };
        let id_table = *(drv.add(8) as *mut u64);
        let probe = *(drv.add(16) as *mut u64);
        let remove = *(drv.add(24) as *mut u64);
        let mut pdrv = PciDriver {
            addr: drv as u64,
            name: [0u8; 64],
            id_table,
            probe,
            remove,
        };
        if !effective_name.is_null() {
            for i in 0..63 {
                let c = *effective_name.add(i);
                pdrv.name[i] = c;
                if c == 0 {
                    break;
                }
            }
        }
        let name_str = core::str::from_utf8(&pdrv.name).unwrap_or("?");
        crate::mesa_println!(
            "[PCI] __pci_register_driver: {} probe={:#x}",
            name_str,
            probe
        );
        PCI_DRIVERS.lock().push(pdrv.clone());
        pci_driver_match_and_probe(&PCI_DRIVERS.lock().last().unwrap());
    }
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_hcd_pci_shutdown(_dev: *mut u8) {}

#[no_mangle]
pub static __shim_usb_hcd_pci_pm_ops: [u8; 64] = [0; 64];

// ── USB HCD Infrastructure ──────────────────────────────────

// Toggle for verbose per-TRB/per-event debug tracing (set true only for deep debugging)
const XHCI_DEBUG: bool = false;

// xHCI TRB constants
const TRB_TYPE_LINK: u32 = 6;
const TRB_TYPE_NOOP: u32 = 8;
const TRB_TYPE_ENABLE_SLOT: u32 = 9;
const TRB_TYPE_DISABLE_SLOT: u32 = 10;
const TRB_TYPE_ADDRESS_DEVICE: u32 = 11;
const TRB_TYPE_CONFIG_EP: u32 = 12;
const TRB_TYPE_RESET_DEV: u32 = 17;
const TRB_CYCLE: u32 = 1;
const TRB_TC: u32 = 1 << 1;
const TRB_IOC: u32 = 1 << 5;
const TRB_TYPE_SHIFT: u32 = 10;

// Transfer TRB constants
const TRB_TYPE_NORMAL: u32 = 1;
const TRB_TYPE_SETUP: u32 = 2;
const TRB_TYPE_DATA: u32 = 3;
const TRB_TYPE_STATUS: u32 = 4;
const EP_TYPE_BULK_OUT: u32 = 2;
const EP_TYPE_BULK_IN: u32 = 3;
const TRB_CH: u32 = 1 << 4;
const TRB_IDT: u32 = 1 << 6;
const TRB_IT_1: u32 = 1 << 10; // Interrupter Target = 1 (we only set up Intr 1)
const TRB_DIR: u32 = 1 << 16;
const TRB_DIR_STATUS: u32 = 1 << 4;
const TRB_ISP: u32 = 1 << 2;

// USB standard request constants
const USB_DIR_IN: u8 = 0x80;
const USB_DIR_OUT: u8 = 0x00;
const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
const USB_DT_DEVICE: u8 = 1;
const USB_DT_CONFIG: u8 = 2;
const USB_DT_INTERFACE: u8 = 4;
const USB_DT_ENDPOINT: u8 = 5;
const USB_CLASS_MASS_STORAGE: u8 = 0x08;
const USB_SUBCLASS_SCSI: u8 = 0x06;
const USB_PROTO_BULK_ONLY: u8 = 0x50;
const USB_ENDPOINT_XFER_BULK: u8 = 0x02;
const USB_ENDPOINT_DIR_MASK: u8 = 0x80;

// BOT (Bulk-Only Transport) constants
const CBW_SIGNATURE: u32 = 0x43425355;
const CSW_SIGNATURE: u32 = 0x53425355;

// SCSI command constants
const SCSI_TEST_UNIT_READY: u8 = 0x00;
const SCSI_REQUEST_SENSE: u8 = 0x03;
const SCSI_INQUIRY: u8 = 0x12;
const SCSI_READ_CAPACITY_10: u8 = 0x25;
const SCSI_READ_10: u8 = 0x28;

// Slot context constants (xHCI 1.0 spec section 6.2.1.1)
// DW0: bits[7:5]=Speed, bits[23:16]=PortNo, bits[26:24]=NumCtx
const SLOT_INFO_SPEED_SHIFT: u32 = 20;
const SLOT_INFO_PORTNO_SHIFT: u32 = 16;
const SLOT_INFO_CONTEXT_ENTRIES_SHIFT: u32 = 27;
// Endpoint context constants (xHCI 1.0 spec section 6.2.2.1)
// DW1: bits[31:16]=MaxPktSize, bits[15:8]=MaxBurst, bits[7:6]=CErr, bits[5:3]=EPType
const EP_INFO_MAX_PKT_SIZE_SHIFT: u32 = 16;
const EP_INFO_TYPE_SHIFT: u32 = 3;
const EP_INFO_TYPE_CONTROL: u32 = 4;
const EP_INFO_MAX_BURST_SHIFT: u32 = 8;
const EP_INFO_CERR_SHIFT: u32 = 6;
const EP_INFO_CERR_3: u32 = 3;

// Port register constants
const PORTSC_CCS: u32 = 1;
const PORTSC_PED: u32 = 1 << 1;
const PORTSC_PR: u32 = 1 << 4;
const PORTSC_PP: u32 = 1 << 9;
const PORTSC_SPEED_SHIFT: u32 = 10;
const PORTSC_CSC: u32 = 1 << 17;
const PORTSC_PLC: u32 = 1 << 22;

// Slot context size (QEMU uses 32-byte offset spacing for contexts)
const SLOT_CTX_SIZE: usize = 32;
const EP_CTX_SIZE: usize = 32;
const INPUT_CTX_SIZE: usize = 128; // 32 ICC + 32 slot + 32 ep0 = 96, rounded to 128 for 64-byte alignment of output context

/// Track probed xHCI controllers for root hub polling
#[derive(Clone, Copy, Debug)]
struct EndpointRing {
    phys: u64,
    virt: *mut u8,
    trb_idx: u32,
    cycle: u32,
}

impl EndpointRing {
    const fn empty() -> Self {
        EndpointRing {
            phys: 0,
            virt: core::ptr::null_mut(),
            trb_idx: 0,
            cycle: 1,
        }
    }
}

struct XhciController {
    mmio: *mut u8,
    op_base: *mut u8,
    db_base: *mut u8,
    rts_base: *mut u8,
    caplength: u8,
    max_ports: u32,
    max_slots: u32,
    hcd: *mut u8,
    bus: u8,
    device: u8,
    function: u8,
    initialized: bool,
    ctrl_idx: usize,
    crcr_off: usize,
    dcbaap_off: usize,
    config_off: usize,
    cmd_ring_phys: u64,
    cmd_ring_virt: *mut u8,
    cmd_enq_idx: u32,
    cmd_cycle: u32,
    evt_ring_phys: u64,
    evt_ring_virt: *mut u8,
    evt_deq_idx: u32,
    evt_cycle: u32,
    evt_mismatch_count: u32, // consecutive cycle mismatches (for rate-limiting)
    erst_phys: u64,
    dcbaa_phys: u64,
    dcbaa_virt: *mut u64,
    port_slot: [u8; 256],
    slot_port: [u8; 32],
    num_devices: u32,
    ep0_ring_phys: u64,
    ep0_ring_virt: *mut u8,
    ep0_trb_idx: u32,
    ep0_cycle: u32,
    ep_out_ring: [EndpointRing; 32], // EP1 OUT per slot_id
    ep_in_ring: [EndpointRing; 32],  // EP1 IN per slot_id
    bulk_ep_configured: [bool; 32],  // whether bulk EPs configured for this slot
    erdp_off: usize, // ERDP register offset: 0x18 for Intr 0, 0x38 for Intr 1 (shifted)
    pub enum_debug: EnumDebugInfo, // stage tracking for enumeration debugging
}

unsafe impl Send for XhciController {}
unsafe impl Sync for XhciController {}

static XHCI_CONTROLLERS: spin::Mutex<alloc::vec::Vec<XhciController>> =
    spin::Mutex::new(alloc::vec::Vec::new());

/// Flush cache lines for a memory range to ensure DMA visibility
unsafe fn xhci_flush_range(addr: *mut u8, len: usize) {
    let start = addr as usize;
    let end = start + len;
    let mut a = start & !63;
    while a < end {
        core::arch::asm!("clflush [{}]", in(reg) a, options(nostack, preserves_flags));
        a += 64;
    }
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

unsafe fn cmd_ring_enqueue(ctrl: &mut XhciController, trb: &[u32; 4]) {
    let idx = ctrl.cmd_enq_idx;
    let slot = ctrl.cmd_ring_virt.add((idx as usize) * 16) as *mut u32;
    let control = trb[3] | (ctrl.cmd_cycle & TRB_CYCLE);
    core::ptr::write_volatile(slot.add(0), trb[0]);
    core::ptr::write_volatile(slot.add(1), trb[1]);
    core::ptr::write_volatile(slot.add(2), trb[2]);
    core::ptr::write_volatile(slot.add(3), control);
    // Flush cache lines to ensure DMA visibility (QEMU TCG reads RAM, not CPU cache)
    xhci_flush_range(slot as *mut u8, 16);
    core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
    if XHCI_DEBUG {
        let rb0 = core::ptr::read_volatile(slot.add(0));
        let rb1 = core::ptr::read_volatile(slot.add(1));
        let rb2 = core::ptr::read_volatile(slot.add(2));
        let rb3 = core::ptr::read_volatile(slot.add(3));
        crate::printk!(
            "[XHCI] CMD enqueue idx={} phys={:#x} trb=[{:#x},{:#x},{:#x},{:#x}]",
            idx,
            ctrl.cmd_ring_phys + (idx as u64) * 16,
            rb0,
            rb1,
            rb2,
            rb3
        );
    }
    ctrl.cmd_enq_idx += 1;
    if ctrl.cmd_enq_idx >= 255 {
        ctrl.cmd_enq_idx = 0;
        ctrl.cmd_cycle ^= 1;
    }
    core::ptr::write_volatile(ctrl.db_base as *mut u32, 0);
}

unsafe fn xhci_ring_doorbell(ctrl: &XhciController, slot_id: u32, epid: u32) {
    // Doorbell array indexed by slot_id, write endpoint ID (1=EP0 OUT)
    let db_reg = ctrl.db_base.add((slot_id as usize) * 4) as *mut u32;
    core::ptr::write_volatile(db_reg, epid);
}

unsafe fn trb_enqueue(ring_virt: *mut u8, idx: u32, cycle: u32, trb: &[u32; 4], phys: u64) {
    let slot = ring_virt.add((idx as usize) * 16) as *mut u32;
    let control = trb[3] | (cycle & TRB_CYCLE);
    core::ptr::write_volatile(slot.add(0), trb[0]);
    core::ptr::write_volatile(slot.add(1), trb[1]);
    core::ptr::write_volatile(slot.add(2), trb[2]);
    core::ptr::write_volatile(slot.add(3), control);
    xhci_flush_range(slot as *mut u8, 16);
    if XHCI_DEBUG {
        crate::printk!(
            "[XHCI] TRB enqueue idx={} phys={:#x} trb=[{:#x},{:#x},{:#x},{:#x}]",
            idx,
            phys + (idx as u64) * 16,
            trb[0],
            trb[1],
            trb[2],
            control
        );
    }
}

unsafe fn wait_for_transfer_event(ctrl: &mut XhciController) -> Option<(u32, u32, u32)> {
    for iter in 0..1000000 {
        while let Some(evt) = process_event_ring(ctrl) {
            let trb_type = (evt[3] >> TRB_TYPE_SHIFT) & 0x3F;
            let compl_code = (evt[2] >> 24) & 0xFF;
            let slot_id = (evt[3] >> 24) & 0xFF;
            if XHCI_DEBUG {
                crate::printk!(
                    "[XHCI] xfer_evt_poll: type={} code={} slot={} evt=[{:#x},{:#x},{:#x},{:#x}] iter={}",
                    trb_type, compl_code, slot_id,
                    evt[0], evt[1], evt[2], evt[3], iter
                );
            }
            if trb_type == 33 {
                continue;
            }
            if trb_type == 32 {
                crate::printk!(
                    "[XHCI] Transfer event: type=32 code={} slot={} leftover={} iter={}",
                    compl_code,
                    slot_id,
                    evt[2] & 0xFFFFFF,
                    iter
                );
                return Some((compl_code, evt[0], evt[2] & 0xFFFFFF));
            }
            if trb_type == 34 {
                continue;
            }
        }
        if XHCI_DEBUG && iter < 5 {
            let deq_slot = ctrl.evt_ring_virt.add((ctrl.evt_deq_idx as usize) * 16) as *const u32;
            crate::printk!(
                "[XHCI] xfer_evt no evt: deq={} cycle={} deq_trb=[{:#x},{:#x},{:#x},{:#x}]",
                ctrl.evt_deq_idx,
                ctrl.evt_cycle,
                core::ptr::read_volatile(deq_slot),
                core::ptr::read_volatile(deq_slot.add(1)),
                core::ptr::read_volatile(deq_slot.add(2)),
                core::ptr::read_volatile(deq_slot.add(3)),
            );
        }
        if iter >= 999990 {
            crate::printk!("[XHCI] TIMEOUT waiting for transfer event");
            return None;
        }
        core::hint::spin_loop();
    }
    None
}

unsafe fn xhci_control_transfer(
    ctrl: &mut XhciController,
    slot_id: u32,
    setup_pkt: &[u8; 8],
    data_buf: *mut u8,
    data_len: u16,
    dir_in: bool,
) -> bool {
    let ep0_ring_virt = ctrl.ep0_ring_virt;
    let ep0_ring_phys = ctrl.ep0_ring_phys;
    let mut idx = ctrl.ep0_trb_idx;
    let mut cycle = ctrl.ep0_cycle;

    let setup_lo = u32::from_le_bytes([setup_pkt[0], setup_pkt[1], setup_pkt[2], setup_pkt[3]]);
    let setup_hi = u32::from_le_bytes([setup_pkt[4], setup_pkt[5], setup_pkt[6], setup_pkt[7]]);

    // Use DMA-coherent buffer for data stage (xHC needs physical address)
    let dma_phys;
    let dma_virt;
    if data_len > 0 {
        let pages = ((data_len as usize) + 4095) / 4096;
        let p = match crate::memory::pmm::alloc_frames(pages) {
            Some(p) => p,
            None => {
                crate::printk!("[XHCI] Ctrl xfer: DMA alloc failed");
                return false;
            }
        };
        dma_phys = p;
        dma_virt = crate::memory::vmm::phys_to_virt(p) as *mut u8;
        if !dir_in {
            core::ptr::copy_nonoverlapping(data_buf, dma_virt, data_len as usize);
        }
    } else {
        dma_phys = 0;
        dma_virt = core::ptr::null_mut();
    }

    // 1. SETUP TRB — TRT field at bits 5:4 (xHCI 1.0)
    //    00b = No Data Stage, 01b = IN, 10b = OUT
    let setup_trt = if data_len > 0 {
        if dir_in {
            TRB_CH // bit 4 = 1, bit 5 = 0 => 01b = IN
        } else {
            TRB_IOC // bit 5 = 1, bit 4 = 0 => 10b = OUT
        }
    } else {
        0
    };
    trb_enqueue(
        ep0_ring_virt,
        idx,
        cycle,
        &[
            setup_lo,
            setup_hi,
            8,
            (TRB_TYPE_SETUP << TRB_TYPE_SHIFT) | TRB_IDT | TRB_IT_1 | setup_trt,
        ],
        ep0_ring_phys,
    );
    idx += 1;
    if idx >= 255 {
        idx = 0;
        cycle ^= 1;
    }

    // 2. DATA TRB (optional, chained) — no ISP per xHCI best practice for control endpoints
    if data_len > 0 {
        let dir_flag = if dir_in { TRB_DIR } else { 0 };
        trb_enqueue(
            ep0_ring_virt,
            idx,
            cycle,
            &[
                dma_phys as u32,
                (dma_phys >> 32) as u32,
                data_len as u32,
                (TRB_TYPE_DATA << TRB_TYPE_SHIFT) | TRB_CH | TRB_IT_1 | dir_flag,
            ],
            ep0_ring_phys,
        );
        idx += 1;
        if idx >= 255 {
            idx = 0;
            cycle ^= 1;
        }
    }

    // 3. STATUS TRB (last, IOC=1)
    // Direction: IN status after OUT data/no-data, OUT status after IN data
    let status_dir = if dir_in { 0 } else { TRB_DIR_STATUS };
    trb_enqueue(
        ep0_ring_virt,
        idx,
        cycle,
        &[
            0,
            0,
            0,
            (TRB_TYPE_STATUS << TRB_TYPE_SHIFT) | TRB_IOC | TRB_IT_1 | status_dir,
        ],
        ep0_ring_phys,
    );
    idx += 1;
    if idx >= 255 {
        idx = 0;
        cycle ^= 1;
    }

    // Compiler + memory barrier: ensure all TRB stores are visible before doorbell
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    // Save idx/cycle before waiting so failure still advances the ring
    ctrl.ep0_trb_idx = idx;
    ctrl.ep0_cycle = cycle;
    if XHCI_DEBUG {
        let ring_virt = ep0_ring_virt;
        for di in 0..4 {
            let d_slot = ring_virt.add(di * 16) as *const u32;
            let d_trb = [
                core::ptr::read_volatile(d_slot.add(0)),
                core::ptr::read_volatile(d_slot.add(1)),
                core::ptr::read_volatile(d_slot.add(2)),
                core::ptr::read_volatile(d_slot.add(3)),
            ];
            crate::printk!(
                "[XHCI] Pre-doorbell TRB[{}]={:#x},{:#x},{:#x},{:#x}",
                di,
                d_trb[0],
                d_trb[1],
                d_trb[2],
                d_trb[3]
            );
        }
        crate::printk!(
            "[XHCI] Ringing doorbell slot={} epid=1 (db_base={:p} + {:#x})",
            slot_id,
            ctrl.db_base,
            slot_id * 4
        );
    }
    core::ptr::write_volatile(ctrl.db_base.add((slot_id as usize) * 4) as *mut u32, 1);
    // Small delay to let QEMU process the doorbell
    for _ in 0..1000 {
        core::hint::spin_loop();
    }
    crate::printk!("[XHCI] Doorbell rung, waiting for transfer event...");

    // Wait for transfer completion event
    let result = wait_for_transfer_event(ctrl);
    if result.is_none() {
        crate::printk!("[XHCI] Ctrl transfer timed out");
        if data_len > 0 {
            crate::memory::pmm::free_frame(dma_phys);
        }
        return false;
    }
    let (compl_code, _trb_addr, _leftover) = result.unwrap();
    if compl_code != 1 {
        crate::printk!("[XHCI] Ctrl transfer failed: code={}", compl_code);
        if data_len > 0 {
            crate::memory::pmm::free_frame(dma_phys);
        }
        return false;
    }

    // Copy data from DMA buffer to caller's buffer (for IN transfers)
    if dir_in && data_len > 0 {
        core::ptr::copy_nonoverlapping(dma_virt, data_buf, data_len as usize);
    }

    if data_len > 0 {
        crate::memory::pmm::free_frame(dma_phys);
    }

    ctrl.ep0_trb_idx = idx;
    ctrl.ep0_cycle = cycle;
    crate::printk!(
        "[XHCI] Ctrl transfer SUCCESS (next idx={} cycle={})",
        idx,
        cycle
    );
    true
}

unsafe fn process_event_ring(ctrl: &mut XhciController) -> Option<[u32; 4]> {
    let evt_ptr = ctrl.evt_ring_virt.add((ctrl.evt_deq_idx as usize) * 16);
    // Flush (invalidate) this cache line before reading — real xHC writes to memory via DMA
    // and our cache may have a stale copy. This ensures we see the xHC's write.
    core::arch::asm!("clflush [{}]", in(reg) evt_ptr, options(nostack, preserves_flags));
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let slot = evt_ptr as *const u32;
    let control = core::ptr::read_volatile(slot.add(3));
    let cycle = control & TRB_CYCLE;
    if cycle != ctrl.evt_cycle {
        ctrl.evt_mismatch_count += 1;
        if ctrl.evt_mismatch_count <= 3 {
            crate::printk!(
                "[XHCI] evt_ring cycle mismatch: deq={} got_cycle={} expected_cycle={} control={:#x} ({} consecutive)",
                ctrl.evt_deq_idx,
                cycle,
                ctrl.evt_cycle,
                control,
                ctrl.evt_mismatch_count
            );
        }
        return None;
    }
    ctrl.evt_mismatch_count = 0;
    let evt = [
        core::ptr::read_volatile(slot.add(0)),
        core::ptr::read_volatile(slot.add(1)),
        core::ptr::read_volatile(slot.add(2)),
        control,
    ];
    if XHCI_DEBUG {
        crate::printk!(
            "[XHCI] evt_ring_read: idx={} cycle={} evt=[{:#x},{:#x},{:#x},{:#x}]",
            ctrl.evt_deq_idx,
            ctrl.evt_cycle,
            evt[0],
            evt[1],
            evt[2],
            evt[3]
        );
    }
    // Advance dequeue pointer
    ctrl.evt_deq_idx = (ctrl.evt_deq_idx + 1) % 256;
    if ctrl.evt_deq_idx == 0 {
        ctrl.evt_cycle ^= 1;
    }
    // Ensure all event data reads complete before updating ERDP (req'd by real xHC)
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let erdp = ctrl.evt_ring_phys + (ctrl.evt_deq_idx as u64) * 16;
    core::ptr::write_volatile(
        ctrl.rts_base.add(ctrl.erdp_off) as *mut u32,
        (erdp as u32) | (ctrl.evt_cycle & 1),
    );
    core::ptr::write_volatile(
        ctrl.rts_base.add(ctrl.erdp_off + 4) as *mut u32,
        (erdp >> 32) as u32,
    );
    // Ensure ERDP write is visible to xHC before we check for more events
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    Some(evt)
}

unsafe fn wait_for_completion(ctrl: &mut XhciController) -> Option<(u32, u32, u32)> {
    for iter in 0..100000 {
        while let Some(evt) = process_event_ring(ctrl) {
            let trb_type = (evt[3] >> TRB_TYPE_SHIFT) & 0x3F;
            let compl_code = (evt[2] >> 24) & 0xFF;
            let slot_id = (evt[3] >> 24) & 0xFF;
            if XHCI_DEBUG {
                crate::printk!(
                    "[XHCI] evt: type={} code={} slot={} iter={}",
                    trb_type,
                    compl_code,
                    slot_id,
                    iter
                );
            }
            // Command Completion Event is type 33 (QEMU: ER_COMMAND_COMPLETE)
            if trb_type == 33 {
                return Some((trb_type, compl_code, slot_id));
            }
            // Port Status Change Event (type 34) — skip, already handled by polling
            if trb_type == 34 {
                continue;
            }
        }
        if iter >= 99990 && XHCI_DEBUG {
            let evt0_ctrl = core::ptr::read_volatile(ctrl.evt_ring_virt.add(12) as *const u32);
            crate::printk!(
                "[XHCI] waiting... iter={} evt_deq_idx={} evt_cycle={} evt_ring[0].ctrl={:#x}",
                iter,
                ctrl.evt_deq_idx,
                ctrl.evt_cycle,
                evt0_ctrl
            );
        }
        core::hint::spin_loop();
    }
    crate::printk!(
        "[XHCI] TIMEOUT: evt_deq_idx={} evt_cycle={} evt_ring_phys={:#x}",
        ctrl.evt_deq_idx,
        ctrl.evt_cycle,
        ctrl.evt_ring_phys
    );
    None
}

unsafe fn port_reset(ctrl: &mut XhciController, port: u32) {
    let portsc = ctrl.op_base.add(0x400 + (port as usize) * 0x10) as *mut u32;
    let v = core::ptr::read_volatile(portsc);
    core::ptr::write_volatile(portsc, v | PORTSC_PR);
    for _ in 0..100000 {
        let v = core::ptr::read_volatile(portsc);
        if (v & PORTSC_PR) == 0 {
            break;
        }
        core::hint::spin_loop();
    }
    // Wait for Port Enabled (PED) after reset completes
    for _ in 0..50000 {
        let v = core::ptr::read_volatile(portsc);
        if v & PORTSC_PED != 0 {
            crate::printk!("[XHCI] Port {} enabled after reset (PORTSC={:#x})", port, v);
            return;
        }
        core::hint::spin_loop();
    }
    crate::printk!(
        "[XHCI] WARNING: Port {} not enabled after reset (PORTSC={:#x})",
        port,
        core::ptr::read_volatile(portsc)
    );
}

unsafe fn port_get_speed(ctrl: &XhciController, port: u32) -> u32 {
    let portsc = ctrl.op_base.add(0x400 + (port as usize) * 0x10) as *const u32;
    (core::ptr::read_volatile(portsc) >> PORTSC_SPEED_SHIFT) & 0xF
}

fn speed_to_max_pkt(speed: u32) -> u32 {
    match speed {
        1 => 8,  // Full-Speed default
        2 => 8,  // Low-Speed
        3 => 64, // High-Speed
        4 => 64, // TEMP: force 64 for SuperSpeed (QEMU device may not support 512)
        _ => 64, // Default to High-Speed
    }
}

unsafe fn allocate_input_context(ctrl: &mut XhciController, slot_id: u32) -> u64 {
    let phys = crate::memory::pmm::alloc_frames(1).unwrap_or(0);
    if phys == 0 {
        return 0;
    }
    let virt = crate::memory::vmm::phys_to_virt(phys) as *mut u8;
    core::ptr::write_bytes(virt, 0, 4096);
    // Input Control Context at offset 0:
    //   Drop Flags = 0, Add Flags = slot(bit0) | ep0(bit1) = 0x3
    core::ptr::write_volatile(virt as *mut u32, 0);
    core::ptr::write_volatile(virt.add(4) as *mut u32, 3);
    // Store DCBAA entry
    core::ptr::write_volatile(
        ctrl.dcbaa_virt.add(slot_id as usize),
        phys + INPUT_CTX_SIZE as u64,
    );
    xhci_flush_range((ctrl.dcbaa_virt.add(slot_id as usize)) as *mut u8, 8);
    phys
}

unsafe fn populate_slot_ctx(input_ctx_virt: *mut u8, speed: u32, _slot_id: u32, port: u32) {
    let slot_ctx = input_ctx_virt.add(32) as *mut u32;
    // DW0: Context Entries (bits 31:27 = 1), Speed (bits 23:20), Route String (bits 19:0 = 0)
    core::ptr::write_volatile(
        slot_ctx.add(0),
        (1 << SLOT_INFO_CONTEXT_ENTRIES_SHIFT) | (speed << SLOT_INFO_SPEED_SHIFT),
    );
    // DW1: Root Hub Port Number (bits 23:16), Interrupter Target (bits 31:24 = 1)
    // Target Interrupter 1 because our event ring is at shifted-offset Interrupter 1
    core::ptr::write_volatile(slot_ctx.add(1), (port << 16) | (1 << 24));
    // DW2: TTT/TT info = 0
    core::ptr::write_volatile(slot_ctx.add(2), 0u32);
    // DW3: Slot State = 0 (set by HC)
    core::ptr::write_volatile(slot_ctx.add(3), 0u32);
}

unsafe fn allocate_ep0_ring(ctrl: &mut XhciController) -> u64 {
    let phys = match crate::memory::pmm::alloc_frames(1) {
        Some(p) => p,
        None => return 0,
    };
    let virt = crate::memory::vmm::phys_to_virt(phys) as *mut u8;
    core::ptr::write_bytes(virt, 0, 4096);
    // Link TRB at index 255 wraps to index 0; cycle toggles (TC=1)
    let link = virt.add(255 * 16) as *mut u32;
    core::ptr::write_volatile(link.add(0), phys as u32);
    core::ptr::write_volatile(link.add(1), (phys >> 32) as u32);
    core::ptr::write_volatile(link.add(2), 0);
    core::ptr::write_volatile(
        link.add(3),
        (TRB_TYPE_LINK << TRB_TYPE_SHIFT) | TRB_TC | TRB_CYCLE,
    );
    ctrl.ep0_ring_phys = phys;
    ctrl.ep0_ring_virt = virt;
    ctrl.ep0_cycle = 1;
    phys
}

unsafe fn populate_ep0_ctx(input_ctx_virt: *mut u8, max_pkt: u32, ring_phys: u64) {
    let ep0_ctx = input_ctx_virt.add(64) as *mut u32;
    // DW0: EP State = 0 (Disabled), Interrupter Target = 1 (bits 31:24)
    core::ptr::write_volatile(ep0_ctx.add(0), 1u32 << 24);
    // DW1: EP Type=Control(4), CErr=3, MaxBurst=0, MaxPktSize
    let ep_info_dw1 = (max_pkt << EP_INFO_MAX_PKT_SIZE_SHIFT)
        | (EP_INFO_TYPE_CONTROL << EP_INFO_TYPE_SHIFT)
        | (EP_INFO_CERR_3 << EP_INFO_CERR_SHIFT);
    core::ptr::write_volatile(ep0_ctx.add(1), ep_info_dw1);
    // DW2/DW3: Dequeue pointer = ring address, DCS=1 in bit 0 of DW2
    core::ptr::write_volatile(ep0_ctx.add(2), (ring_phys as u32) | 1);
    core::ptr::write_volatile(ep0_ctx.add(3), (ring_phys >> 32) as u32);
    // DW4: Average TRB Length - leave 0
}

unsafe fn allocate_endpoint_ring() -> EndpointRing {
    let phys = match crate::memory::pmm::alloc_frames(1) {
        Some(p) => p,
        None => return EndpointRing::empty(),
    };
    let virt = crate::memory::vmm::phys_to_virt(phys) as *mut u8;
    core::ptr::write_bytes(virt, 0, 4096);
    let link = virt.add(255 * 16) as *mut u32;
    core::ptr::write_volatile(link.add(0), phys as u32);
    core::ptr::write_volatile(link.add(1), (phys >> 32) as u32);
    core::ptr::write_volatile(link.add(2), 0);
    core::ptr::write_volatile(
        link.add(3),
        (TRB_TYPE_LINK << TRB_TYPE_SHIFT) | TRB_TC | TRB_CYCLE,
    );
    EndpointRing {
        phys,
        virt,
        trb_idx: 0,
        cycle: 1,
    }
}

unsafe fn configure_bulk_endpoints(ctrl: &mut XhciController, slot_id: u32, max_pkt: u32) -> bool {
    let sid = slot_id as usize;
    // Allocate rings for EP1 OUT (epid=2) and EP1 IN (epid=3)
    let out_ring = allocate_endpoint_ring();
    if out_ring.phys == 0 {
        crate::printk!("[XHCI] Failed to allocate EP1 OUT ring");
        return false;
    }
    let in_ring = allocate_endpoint_ring();
    if in_ring.phys == 0 {
        crate::memory::pmm::free_frame(out_ring.phys);
        crate::printk!("[XHCI] Failed to allocate EP1 IN ring");
        return false;
    }
    ctrl.ep_out_ring[sid] = out_ring;
    ctrl.ep_in_ring[sid] = in_ring;

    // Allocate input context page for Configure Endpoint
    let ctx_phys = match crate::memory::pmm::alloc_frames(1) {
        Some(p) => p,
        None => {
            crate::printk!("[XHCI] Failed to alloc ctx for Configure EP");
            return false;
        }
    };
    let ctx_virt = crate::memory::vmm::phys_to_virt(ctx_phys) as *mut u8;
    core::ptr::write_bytes(ctx_virt, 0, 4096);

    // ICC: Drop=0, Add=slot(0) | ep1_out(2) | ep1_in(3) = 0xD
    core::ptr::write_volatile(ctx_virt as *mut u32, 0u32);
    core::ptr::write_volatile(ctx_virt.add(4) as *mut u32, 0xDu32);

    // Slot context at offset 32: update Context Entries to 3 (slot + ep1out + ep1in)
    {
        let sc = ctx_virt.add(32) as *mut u32;
        // Read current speed/port from the DCBAA output context
        let dev_ctx_virt =
            crate::memory::vmm::phys_to_virt(core::ptr::read_volatile(ctrl.dcbaa_virt.add(sid)))
                as *mut u8;
        let out_slot0 = core::ptr::read_volatile(dev_ctx_virt as *const u32);
        // Mask out Context Entries bits (31:27), set to 3
        let slot_info = (out_slot0 & !(0x1F << 27)) | (3 << SLOT_INFO_CONTEXT_ENTRIES_SHIFT);
        core::ptr::write_volatile(sc.add(0), slot_info);
        // Copy DW1 (port number, interrupter target) from output context
        let out_slot1 = core::ptr::read_volatile(dev_ctx_virt.add(4) as *const u32);
        core::ptr::write_volatile(sc.add(1), out_slot1);
        core::ptr::write_volatile(sc.add(2), 0u32);
        core::ptr::write_volatile(sc.add(3), 0u32);
    }

    // EP1 OUT context at offset 64 (second endpoint context entry, Add bit 2)
    {
        let ep1_out = ctx_virt.add(64) as *mut u32;
        core::ptr::write_volatile(ep1_out.add(0), 1u32 << 24); // Interrupter Target=1
        let ep_info = (max_pkt << EP_INFO_MAX_PKT_SIZE_SHIFT)
            | (EP_TYPE_BULK_OUT << EP_INFO_TYPE_SHIFT)
            | (EP_INFO_CERR_3 << EP_INFO_CERR_SHIFT);
        core::ptr::write_volatile(ep1_out.add(1), ep_info);
        core::ptr::write_volatile(ep1_out.add(2), (out_ring.phys as u32) | 1); // DCS=1
        core::ptr::write_volatile(ep1_out.add(3), (out_ring.phys >> 32) as u32);
        core::ptr::write_volatile(ep1_out.add(4), 0u32);
    }

    // EP1 IN context at offset 96 (third endpoint context entry, Add bit 3)
    {
        let ep1_in = ctx_virt.add(96) as *mut u32;
        core::ptr::write_volatile(ep1_in.add(0), 1u32 << 24); // Interrupter Target=1
        let ep_info = (max_pkt << EP_INFO_MAX_PKT_SIZE_SHIFT)
            | (EP_TYPE_BULK_IN << EP_INFO_TYPE_SHIFT)
            | (EP_INFO_CERR_3 << EP_INFO_CERR_SHIFT);
        core::ptr::write_volatile(ep1_in.add(1), ep_info);
        core::ptr::write_volatile(ep1_in.add(2), (in_ring.phys as u32) | 1); // DCS=1
        core::ptr::write_volatile(ep1_in.add(3), (in_ring.phys >> 32) as u32);
        core::ptr::write_volatile(ep1_in.add(4), 0u32);
    }

    // Flush context
    xhci_flush_range(ctx_virt, 192);

    // Store output context (device context) at offset 192
    core::ptr::write_volatile(ctrl.dcbaa_virt.add(sid), ctx_phys + 192);
    xhci_flush_range(ctrl.dcbaa_virt.add(sid) as *mut u8, 8);

    // Configure Endpoint command (DB=0, type=12, slot_id in control bits)
    let cmd_trb = [
        ctx_phys as u32,
        (ctx_phys >> 32) as u32,
        0,
        (TRB_TYPE_CONFIG_EP << TRB_TYPE_SHIFT) | TRB_IT_1 | (slot_id << 24) | TRB_IOC,
    ];
    cmd_ring_enqueue(ctrl, &cmd_trb);
    let result = wait_for_completion(ctrl);
    if result.is_none() {
        crate::printk!("[XHCI] Slot {}: Configure Endpoint timed out", slot_id);
        return false;
    }
    let (trb_type, compl_code, _) = result.unwrap();
    if trb_type != 33 || compl_code != 1 {
        crate::printk!(
            "[XHCI] Slot {}: Configure Endpoint failed: type={} code={}",
            slot_id,
            trb_type,
            compl_code
        );
        return false;
    }
    crate::printk!(
        "[XHCI] Slot {}: Configure Endpoint SUCCESS (EP1 OUT/IN)",
        slot_id
    );
    ctrl.bulk_ep_configured[sid] = true;
    true
}

unsafe fn xhci_bulk_transfer(
    ctrl: &mut XhciController,
    slot_id: u32,
    epid: u32,
    data_buf: *mut u8,
    data_len: u32,
) -> bool {
    let sid = slot_id as usize;
    let dir_in = epid == 3;

    // Copy ring fields to avoid simultaneous mutable borrow with wait_for_transfer_event
    let (ring_phys, ring_virt, mut t_idx, mut t_cycle) = if epid == 2 {
        let r = &ctrl.ep_out_ring[sid];
        (r.phys, r.virt, r.trb_idx, r.cycle)
    } else {
        let r = &ctrl.ep_in_ring[sid];
        (r.phys, r.virt, r.trb_idx, r.cycle)
    };

    if ring_phys == 0 {
        crate::printk!(
            "[XHCI] Bulk ring not allocated for slot {} epid {}",
            slot_id,
            epid
        );
        return false;
    }

    // Allocate DMA buffer for data
    let dma_phys;
    let dma_virt;
    if data_len > 0 {
        let pages = ((data_len as usize) + 4095) / 4096;
        let p = match crate::memory::pmm::alloc_frames(pages) {
            Some(p) => p,
            None => {
                crate::printk!("[XHCI] Bulk xfer: DMA alloc failed");
                return false;
            }
        };
        dma_phys = p;
        dma_virt = crate::memory::vmm::phys_to_virt(p) as *mut u8;
        if !dir_in {
            core::ptr::copy_nonoverlapping(data_buf, dma_virt, data_len as usize);
        }
    } else {
        dma_phys = 0;
        dma_virt = core::ptr::null_mut();
    }

    // Normal TRB for bulk data
    trb_enqueue(
        ring_virt,
        t_idx,
        t_cycle,
        &[
            dma_phys as u32,
            (dma_phys >> 32) as u32,
            data_len,
            (TRB_TYPE_NORMAL << TRB_TYPE_SHIFT) | TRB_IOC | TRB_IT_1,
        ],
        ring_phys,
    );
    t_idx += 1;
    if t_idx >= 255 {
        t_idx = 0;
        t_cycle ^= 1;
    }

    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    if XHCI_DEBUG {
        crate::printk!(
            "[XHCI] Bulk xfer: ringing doorbell slot={} epid={}",
            slot_id,
            epid
        );
    }
    core::ptr::write_volatile(ctrl.db_base.add((slot_id as usize) * 4) as *mut u32, epid);
    for _ in 0..1000 {
        core::hint::spin_loop();
    }

    let result = wait_for_transfer_event(ctrl);
    if result.is_none() {
        crate::printk!(
            "[XHCI] Bulk xfer timed out (slot={} epid={})",
            slot_id,
            epid
        );
        if data_len > 0 {
            crate::memory::pmm::free_frame(dma_phys);
        }
        return false;
    }
    let (compl_code, _trb_addr, _leftover) = result.unwrap();
    if compl_code != 1 {
        crate::printk!("[XHCI] Bulk xfer failed: code={}", compl_code);
        if data_len > 0 {
            crate::memory::pmm::free_frame(dma_phys);
        }
        return false;
    }

    if dir_in && data_len > 0 {
        core::ptr::copy_nonoverlapping(dma_virt, data_buf, data_len as usize);
    }

    if data_len > 0 {
        crate::memory::pmm::free_frame(dma_phys);
    }

    // Write back ring state
    if epid == 2 {
        ctrl.ep_out_ring[sid].trb_idx = t_idx;
        ctrl.ep_out_ring[sid].cycle = t_cycle;
    } else {
        ctrl.ep_in_ring[sid].trb_idx = t_idx;
        ctrl.ep_in_ring[sid].cycle = t_cycle;
    }
    crate::printk!(
        "[XHCI] Bulk xfer SUCCESS (slot={} epid={} len={})",
        slot_id,
        epid,
        data_len
    );
    true
}

unsafe fn enumerate_device(ctrl: &mut XhciController, port: u32) {
    let slot_id = ctrl.num_devices + 1;
    if slot_id >= ctrl.max_slots || slot_id >= 32 {
        ctrl.enum_debug.set_error("max slots reached");
        crate::printk!("[XHCI] Cannot enumerate: max slots reached");
        return;
    }

    ctrl.enum_debug = EnumDebugInfo::new(); // fresh start for new enumeration
    ctrl.enum_debug.retry_count = 0;
    ctrl.enum_debug.set_stage(EnumStage::PortReset);

    // 1. Reset the port first (waits for PED internally)
    port_reset(ctrl, port);

    // Read speed AFTER port reset — device speed is finalized after reset
    let speed = port_get_speed(ctrl, port);
    let max_pkt = speed_to_max_pkt(speed);
    crate::printk!("[XHCI] Port {}: speed={} max_pkt={}", port, speed, max_pkt);

    ctrl.enum_debug.set_stage(EnumStage::EnableSlot);

    // 2. Enable Slot command
    let cmd_trb = [
        0u32,
        0u32,
        0u32,
        (TRB_TYPE_ENABLE_SLOT << TRB_TYPE_SHIFT) | TRB_IT_1 | TRB_IOC,
    ];
    cmd_ring_enqueue(ctrl, &cmd_trb);
    let result = wait_for_completion(ctrl);
    if result.is_none() {
        ctrl.enum_debug.set_error("Enable Slot timed out");
        crate::printk!("[XHCI] Port {}: Enable Slot timed out", port);
        return;
    }
    let (trb_type, compl_code, got_slot) = result.unwrap();
    if trb_type != 33 || compl_code != 1 || got_slot == 0 {
        ctrl.enum_debug.set_error(&alloc::format!(
            "Enable Slot failed: type={} code={} slot={}",
            trb_type,
            compl_code,
            got_slot
        ));
        crate::printk!(
            "[XHCI] Port {}: Enable Slot failed: type={} code={} slot={}",
            port,
            trb_type,
            compl_code,
            got_slot
        );
        return;
    }
    crate::printk!("[XHCI] Port {}: Enable Slot -> slot_id={}", port, got_slot);

    // 3. Allocate and populate Input Context
    let input_ctx_phys = allocate_input_context(ctrl, got_slot);
    if input_ctx_phys == 0 {
        crate::printk!("[XHCI] Port {}: Failed to allocate input context", port);
        return;
    }
    let input_ctx_virt = crate::memory::vmm::phys_to_virt(input_ctx_phys) as *mut u8;
    populate_slot_ctx(input_ctx_virt, speed, got_slot, port + 1);
    // Allocate EP0 transfer ring (required by spec for Address Device)
    let ep0_ring_phys = allocate_ep0_ring(ctrl);
    if ep0_ring_phys == 0 {
        crate::printk!("[XHCI] Port {}: Failed to allocate EP0 ring", port);
        return;
    }
    crate::printk!("[XHCI] EP0 ring at {:#x}", ep0_ring_phys);
    populate_ep0_ctx(input_ctx_virt, max_pkt, ep0_ring_phys);

    // Flush input context and EP0 ring cache lines for DMA visibility
    xhci_flush_range(input_ctx_virt, 128);
    xhci_flush_range(
        crate::memory::vmm::phys_to_virt(ep0_ring_phys) as *mut u8,
        64,
    );

    // Debug: dump Input Context, DCBAA, and Address Device TRB
    crate::printk!("[XHCI] === Address Device debug ===");
    let icc0 = core::ptr::read_volatile(input_ctx_virt as *const u32);
    let icc1 = core::ptr::read_volatile(input_ctx_virt.add(4) as *const u32);
    let sc0 = core::ptr::read_volatile(input_ctx_virt.add(32) as *const u32);
    let sc1 = core::ptr::read_volatile(input_ctx_virt.add(36) as *const u32);
    let sc2 = core::ptr::read_volatile(input_ctx_virt.add(40) as *const u32);
    let sc3 = core::ptr::read_volatile(input_ctx_virt.add(44) as *const u32);
    let ec0 = core::ptr::read_volatile(input_ctx_virt.add(64) as *const u32);
    let ec1 = core::ptr::read_volatile(input_ctx_virt.add(68) as *const u32);
    let ec2 = core::ptr::read_volatile(input_ctx_virt.add(72) as *const u32);
    let ec3 = core::ptr::read_volatile(input_ctx_virt.add(76) as *const u32);
    let ec4 = core::ptr::read_volatile(input_ctx_virt.add(80) as *const u32);
    let dcbaa_entry = core::ptr::read_volatile(ctrl.dcbaa_virt.add(got_slot as usize));
    crate::printk!("[XHCI] IC physical={:#x}", input_ctx_phys);
    crate::printk!("[XHCI] ICC=[{:#x},{:#x}]", icc0, icc1);
    crate::printk!(
        "[XHCI] slot_ctx=[{:#x},{:#x},{:#x},{:#x}]",
        sc0,
        sc1,
        sc2,
        sc3
    );
    crate::printk!(
        "[XHCI] ep0_ctx=[{:#x},{:#x},{:#x},{:#x},{:#x}]",
        ec0,
        ec1,
        ec2,
        ec3,
        ec4
    );
    crate::printk!("[XHCI] DCBAA[{}]={:#x}", got_slot, dcbaa_entry);
    crate::printk!("[XHCI] DCBAA base={:#x}", ctrl.dcbaa_phys);
    crate::printk!("[XHCI] DB base={:p}", ctrl.db_base);
    // Read back CRCR (using detected offset) to verify it points to our command ring
    let crcr_lo = core::ptr::read_volatile(ctrl.op_base.add(ctrl.crcr_off) as *const u32);
    let crcr_hi = core::ptr::read_volatile(ctrl.op_base.add(ctrl.crcr_off + 4) as *const u32);
    crate::printk!(
        "[XHCI] CRCR before AddrDev={:#x}{:#x} (off=+{:#x})",
        crcr_hi,
        crcr_lo,
        ctrl.crcr_off
    );
    crate::printk!(
        "[XHCI] Expected CRCR={:#x} (cmd_ring_phys)",
        ctrl.cmd_ring_phys
    );

    ctrl.enum_debug.set_stage(EnumStage::AddressDevice);

    // 4. Address Device command
    // QEMU reads slot_id from control bits 31:24 (via xhci_get_slot)
    let addr_trb = [
        input_ctx_phys as u32,
        (input_ctx_phys >> 32) as u32,
        0,
        (TRB_TYPE_ADDRESS_DEVICE << TRB_TYPE_SHIFT) | TRB_IT_1 | (got_slot << 24) | TRB_IOC,
    ];
    cmd_ring_enqueue(ctrl, &cmd_trb);
    let result = wait_for_completion(ctrl);
    if result.is_none() {
        crate::printk!("[XHCI] Port {}: Address Device timed out", port);
        return;
    }
    let (trb_type, compl_code, _) = result.unwrap();
    if trb_type != 33 || compl_code != 1 {
        crate::printk!(
            "[XHCI] Port {}: Address Device failed: type={} code={}",
            port,
            trb_type,
            compl_code
        );
        return;
    }
    crate::printk!(
        "[XHCI] Port {}: Address Device SUCCESS slot_id={}",
        port,
        got_slot
    );

    // Read back output device context to verify slot/EP0 state
    let dev_ctx_virt =
        crate::memory::vmm::phys_to_virt(input_ctx_phys + INPUT_CTX_SIZE as u64) as *mut u8;
    let out_slot = core::ptr::read_volatile(dev_ctx_virt.add(0) as *const u32);
    let out_slot1 = core::ptr::read_volatile(dev_ctx_virt.add(4) as *const u32);
    let out_slot3 = core::ptr::read_volatile(dev_ctx_virt.add(12) as *const u32);
    let out_ep0_0 = core::ptr::read_volatile(dev_ctx_virt.add(32) as *const u32);
    let out_ep0_1 = core::ptr::read_volatile(dev_ctx_virt.add(36) as *const u32);
    let out_ep0_2 = core::ptr::read_volatile(dev_ctx_virt.add(40) as *const u32);
    crate::printk!(
        "[XHCI] Output ctx: slot=[{:#x},{:#x},_,{:#x}] ep0=[{:#x},{:#x},{:#x},_]",
        out_slot,
        out_slot1,
        out_slot3,
        out_ep0_0,
        out_ep0_1,
        out_ep0_2
    );
    crate::printk!(
        "[XHCI] Slot state={} EP0 state={}",
        (out_slot >> 27) & 0x1F,
        out_ep0_0 & 0x1F
    );

    // 5. Register the device
    ctrl.port_slot[port as usize] = got_slot as u8;
    ctrl.slot_port[got_slot as usize] = port as u8;
    ctrl.num_devices += 1;
    crate::printk!(
        "[XHCI] Device on port {} enumerated: slot={} speed={} max_pkt={}",
        port,
        got_slot,
        speed,
        max_pkt
    );

    // 6. Skip zero-length test (may interfere with device state on QEMU)
    //    Go directly to device descriptor read.

    // Dump EP0 context dequeue pointer to verify xHC advanced after Address Device
    {
        let dev_ctx_base = crate::memory::vmm::phys_to_virt(core::ptr::read_volatile(
            ctrl.dcbaa_virt.add(got_slot as usize),
        )) as *mut u8;
        let ep0_dw2 = core::ptr::read_volatile(dev_ctx_base.add(40) as *const u32);
        crate::printk!(
            "[XHCI] EP0 dequeue ptr before descriptor read: {:#x} (DCS={})",
            ep0_dw2 & !1,
            ep0_dw2 & 1
        );
    }

    // 7. Read Device Descriptor
    let mut dev_desc_buf: [u8; 18] = [0u8; 18];
    ctrl.enum_debug.set_stage(EnumStage::DeviceDescriptor);
    let setup_dev_desc = [
        USB_DIR_IN,
        USB_REQ_GET_DESCRIPTOR,
        0,
        USB_DT_DEVICE,
        0,
        0,
        18,
        0,
    ];
    let desc_success = if xhci_control_transfer(
        ctrl,
        got_slot,
        &setup_dev_desc,
        dev_desc_buf.as_mut_ptr(),
        18,
        true,
    ) {
        crate::printk!(
            "[XHCI] Device Descriptor: bcdUSB={:02x}.{:02x} bDeviceClass={:#x} bDeviceSubClass={:#x} bMaxPacketSize0={} idVendor={:#x} idProduct={:#x} bcdDevice={:02x}.{:02x}",
            dev_desc_buf[3], dev_desc_buf[2],
            dev_desc_buf[4], dev_desc_buf[5],
            dev_desc_buf[7],
            (dev_desc_buf[9] as u16) << 8 | dev_desc_buf[8] as u16,
            (dev_desc_buf[11] as u16) << 8 | dev_desc_buf[10] as u16,
            dev_desc_buf[13], dev_desc_buf[12],
        );
        true
    } else {
        crate::printk!("[XHCI] Device Descriptor read FAILED");
        false
    };

    if desc_success {
        let speed = port_get_speed(ctrl, port);
        let dev_addr = usb_register_device(ctrl.ctrl_idx, got_slot, port + 1, speed, &dev_desc_buf);
        crate::printk!("[XHCI] Device registered at {:p}", dev_addr);
    }

    // 8. Read full Configuration Descriptor
    ctrl.enum_debug.set_stage(EnumStage::ConfigDescriptor);
    let mut cfg_hdr: [u8; 9] = [0u8; 9];
    let setup_cfg_hdr = [
        USB_DIR_IN,
        USB_REQ_GET_DESCRIPTOR,
        0,
        USB_DT_CONFIG,
        0,
        0,
        9,
        0,
    ];
    if xhci_control_transfer(
        ctrl,
        got_slot,
        &setup_cfg_hdr,
        cfg_hdr.as_mut_ptr(),
        9,
        true,
    ) {
        crate::printk!(
            "[XHCI] Config Descriptor: num_ifaces={} total_len={}",
            cfg_hdr[4],
            (cfg_hdr[7] as u16) << 8 | cfg_hdr[6] as u16
        );
    } else {
        crate::printk!("[XHCI] Config Descriptor header read FAILED");
    }

    // 9. Configure bulk endpoints EP1 OUT and EP1 IN
    ctrl.enum_debug.set_stage(EnumStage::BulkConfig);
    let max_pkt_for_speed = speed_to_max_pkt(speed);
    let bulk_ok = if desc_success {
        configure_bulk_endpoints(ctrl, got_slot, max_pkt_for_speed)
    } else {
        false
    };
    crate::printk!(
        "[XHCI] Bulk endpoints configured: {}",
        if bulk_ok { "SUCCESS" } else { "SKIPPED/FAILED" }
    );

    // 10. Probe USB class drivers
    if desc_success && bulk_ok {
        let vendor_id = (dev_desc_buf[9] as u16) << 8 | dev_desc_buf[8] as u16;
        let product_id = (dev_desc_buf[11] as u16) << 8 | dev_desc_buf[10] as u16;
        let dev_addr = {
            let records = USB_DEVICE_RECORDS.lock();
            records.iter().find_map(|(&addr, rec)| {
                if rec.ctrl_idx == ctrl.ctrl_idx && rec.slot_id == got_slot {
                    Some(addr as *mut u8)
                } else {
                    None
                }
            })
        };
        if let Some(dev_ptr) = dev_addr {
            let drvs = USB_DRIVERS.lock();
            for drv in drvs.iter() {
                if drv.id_table == 0 {
                    continue;
                }
                // Iterate id_table entries
                let mut entry_idx = 0;
                loop {
                    let entry = (drv.id_table + entry_idx * 24) as *const u8;
                    let match_flags = u16::from_le(*(entry as *const u16));
                    if match_flags == 0 {
                        break; // end of table
                    }
                    let id_vendor = u16::from_le(*(entry.add(2) as *const u16));
                    let id_product = u16::from_le(*(entry.add(4) as *const u16));
                    let mut id_match =
                        (match_flags & USB_DEVICE_ID_MATCH_VENDOR) == 0 || id_vendor == vendor_id;
                    id_match = id_match
                        && ((match_flags & USB_DEVICE_ID_MATCH_PRODUCT) == 0
                            || id_product == product_id);
                    if id_match {
                        let name_str = core::str::from_utf8(&drv.name).unwrap_or("?");
                        crate::printk!(
                            "[USB] Driver {} matches device {:04x}:{:04x}, calling probe",
                            name_str,
                            vendor_id,
                            product_id
                        );
                        // Create fake usb_interface at dev+0x100
                        let intf = dev_ptr.add(0x100);
                        USB_INTF_TO_DEV.lock().insert(intf as u64, dev_ptr as u64);
                        let probe_fn: extern "C" fn(*mut u8) -> i32 =
                            core::mem::transmute(drv.probe);
                        let probe_ret = probe_fn(intf);
                        crate::printk!("[USB] Driver {} probe returned {}", name_str, probe_ret);
                        break;
                    }
                    entry_idx += 1;
                }
            }
        }

        // 11. Probe native USB mass storage driver
        ctrl.enum_debug.set_stage(EnumStage::StorageProbe);
        crate::printk!("[XHCI] Probing native USB mass storage driver...");
        usb_storage_probe(ctrl, got_slot, port);

        ctrl.enum_debug.set_stage(EnumStage::Complete);
    }
}

unsafe fn disable_slot(ctrl: &mut XhciController, slot_id: u32) {
    // QEMU reads slot_id from control bits 31:24 (via xhci_get_slot)
    let cmd_trb = [
        0u32,
        0u32,
        0,
        (TRB_TYPE_DISABLE_SLOT << TRB_TYPE_SHIFT) | TRB_IT_1 | (slot_id << 24) | TRB_IOC,
    ];
    cmd_ring_enqueue(ctrl, &cmd_trb);
    let _ = wait_for_completion(ctrl);
    crate::printk!("[XHCI] Slot {} disabled", slot_id);
}

/// Poll all probed xHCI controllers for port status changes.
/// Called periodically from the main loop.
pub fn poll_xhci_controllers() {
    let mut controllers = XHCI_CONTROLLERS.lock();
    for ctrl in controllers.iter_mut() {
        if ctrl.mmio.is_null() || !ctrl.initialized {
            continue;
        }
        // Check if this controller's event ring was stolen by another probe (HCRST)
        // by comparing ERSTBA in the runtime registers with our saved erst_phys.
        if ctrl.erst_phys != 0 {
            unsafe {
                let erstba_off = if ctrl.erdp_off >= 8 {
                    ctrl.erdp_off - 8
                } else {
                    8_usize
                };
                let current_erstba =
                    core::ptr::read_volatile(ctrl.rts_base.add(erstba_off) as *const u64);
                if current_erstba != 0 && current_erstba != ctrl.erst_phys {
                    crate::printk!(
                        "[XHCI] Ctrl {} ring stolen (ERSTBA@{:#x} {:#x} != {:#x})",
                        ctrl.ctrl_idx,
                        erstba_off,
                        current_erstba,
                        ctrl.erst_phys
                    );
                    ctrl.initialized = false;
                    continue;
                }
            }
        }
        unsafe {
            for port in 0..ctrl.max_ports {
                let portsc_offset = 0x400 + (port as usize) * 0x10;
                let portsc =
                    core::ptr::read_volatile(ctrl.op_base.add(portsc_offset) as *const u32);
                let ccs = (portsc & PORTSC_CCS) != 0;
                let csc = (portsc >> 17) & 0x01;
                let has_slot = ctrl.port_slot[port as usize] != 0;

                // Enumerate if: (a) CSC is set (hotplug), OR (b) CCS=1 but no slot yet (already connected at boot)
                let should_handle = csc != 0 || (ccs && !has_slot);

                if !should_handle {
                    continue;
                }

                if ccs {
                    // Device connected
                    if csc != 0 {
                        // Clear CSC status bit by writing 1
                        let w_portsc = portsc | PORTSC_CSC;
                        core::ptr::write_volatile(
                            ctrl.op_base.add(portsc_offset) as *mut u32,
                            w_portsc,
                        );
                    }
                    crate::printk!(
                        "[XHCI] Port {}: device connected (CCS=1 CSC={}) enumerating...",
                        port,
                        csc
                    );
                    enumerate_device(ctrl, port);
                } else {
                    // Device disconnected (CSC=1 and CCS=0)
                    // Clear CSC
                    let w_portsc = portsc | PORTSC_CSC;
                    core::ptr::write_volatile(
                        ctrl.op_base.add(portsc_offset) as *mut u32,
                        w_portsc,
                    );
                    let slot_id = ctrl.port_slot[port as usize];
                    if slot_id != 0 {
                        crate::printk!(
                            "[XHCI] Port {}: device disconnected (slot {})",
                            port,
                            slot_id
                        );
                        disable_slot(ctrl, slot_id as u32);
                        ctrl.port_slot[port as usize] = 0;
                        ctrl.slot_port[slot_id as usize] = 0;
                        core::ptr::write_volatile(ctrl.dcbaa_virt.add(slot_id as usize), 0);
                        if ctrl.num_devices > 0 {
                            ctrl.num_devices -= 1;
                        }
                    }
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn __shim_usb_create_hcd(
    _driver: *const u8,
    _dev: *mut u8,
    _bus_name: *const u8,
) -> *mut u8 {
    unsafe { __shim_kzalloc(4096, 0x2000) }
}

#[no_mangle]
pub extern "C" fn __shim_usb_add_hcd(hcd: *mut u8, _irqnum: u32, _irqflags: u32) -> i32 {
    if hcd.is_null() {
        return -22;
    }
    crate::printk!("[USB] usb_add_hcd: hcd={:p} (stub)", hcd);
    0
}

#[no_mangle]
pub extern "C" fn __shim_usb_remove_hcd(_hcd: *mut u8) {
    crate::printk!("[USB] usb_remove_hcd: stub");
}

#[no_mangle]
pub extern "C" fn __shim_usb_put_hcd(_hcd: *mut u8) {}

/// Free DMA pages allocated for xHCI controller
unsafe fn xhci_free_dma(ctrl: &XhciController) {
    if ctrl.cmd_ring_phys != 0 {
        crate::memory::pmm::free_frame(ctrl.cmd_ring_phys);
    }
    if ctrl.evt_ring_phys != 0 {
        crate::memory::pmm::free_frame(ctrl.evt_ring_phys);
    }
    if ctrl.erst_phys != 0 {
        crate::memory::pmm::free_frame(ctrl.erst_phys);
    }
    if ctrl.dcbaa_phys != 0 {
        crate::memory::pmm::free_frame(ctrl.dcbaa_phys);
    }
}

/// Auto-detect operational register layout: QEMU vs standard xHCI.
/// QEMU uses non-standard offsets (CRCR=0x18, DCBAAP=0x30, CONFIG=0x38).
/// Standard xHCI uses (CRCR=0x10, DCBAAP=0x18, CONFIG=0x20).
/// Returns (crcr_off, dcbaap_off, config_off).
unsafe fn detect_xhci_reg_layout(op_base: *mut u8) -> (usize, usize, usize) {
    // Write test pattern to QEMU CONFIG offset (op+0x38)
    let test_val = 0xAu32;
    core::ptr::write_volatile(op_base.add(0x38) as *mut u32, test_val);
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let readback = core::ptr::read_volatile(op_base.add(0x38) as *const u32);
    if readback & 0xFF == test_val {
        crate::printk!(
            "[XHCI] Detected QEMU register layout (CRCR=0x18, DCBAAP=0x30, CONFIG=0x38)"
        );
        // Restore 0
        core::ptr::write_volatile(op_base.add(0x38) as *mut u32, 0);
        return (0x18, 0x30, 0x38);
    }
    // Try standard CONFIG offset (op+0x20)
    core::ptr::write_volatile(op_base.add(0x20) as *mut u32, test_val);
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let readback = core::ptr::read_volatile(op_base.add(0x20) as *const u32);
    if readback & 0xFF == test_val {
        crate::printk!(
            "[XHCI] Detected standard xHCI register layout (CRCR=0x10, DCBAAP=0x18, CONFIG=0x20)"
        );
        core::ptr::write_volatile(op_base.add(0x20) as *mut u32, 0);
        return (0x10, 0x18, 0x20);
    }
    // Default to QEMU layout
    crate::printk!("[XHCI] Register layout detection ambiguous, defaulting to QEMU layout");
    (0x18, 0x30, 0x38)
}

/// Halt the xHCI controller if it is currently running.
unsafe fn halt_xhci_controller(op_base: *mut u8) {
    let usbcmd = core::ptr::read_volatile(op_base as *const u32);
    if usbcmd & 1 == 0 {
        return;
    }
    crate::printk!("[XHCI] Halting running controller (USBCMD={:#x})", usbcmd);
    core::ptr::write_volatile(op_base as *mut u32, usbcmd & !1);
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    for i in 0..10000 {
        let usbsts = core::ptr::read_volatile(op_base.add(4) as *const u32);
        if usbsts & 1 != 0 {
            crate::printk!("[XHCI] Controller halted ({} iterations)", i);
            return;
        }
        core::hint::spin_loop();
    }
    crate::printk!("[XHCI] WARNING: Controller did not halt, continuing anyway");
}

/// Stop the command ring if it is running (CRR=1).
unsafe fn stop_xhci_command_ring(op_base: *mut u8, crcr_off: usize) {
    let crcr = core::ptr::read_volatile(op_base.add(crcr_off) as *const u32);
    if crcr & (1 << 3) == 0 {
        return;
    }
    crate::printk!("[XHCI] Stopping command ring (CRR=1)");
    core::ptr::write_volatile(op_base.add(crcr_off) as *mut u32, 1 << 1);
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    for i in 0..10000 {
        let crr = core::ptr::read_volatile(op_base.add(crcr_off) as *const u32);
        if crr & (1 << 3) == 0 {
            crate::printk!("[XHCI] Command ring stopped ({} iterations)", i);
            return;
        }
        core::hint::spin_loop();
    }
    crate::printk!("[XHCI] WARNING: Command ring did not stop, continuing anyway");
}

/// Perform Host Controller Reset (HCRST). Must be called while controller is halted.
/// After HCRST, all operational registers return to default states.
unsafe fn xhci_hcrst(op_base: *mut u8) -> bool {
    crate::printk!("[XHCI] Performing HCRST...");
    // Write USBCMD[1] = HCRST
    let usbcmd = core::ptr::read_volatile(op_base as *const u32);
    core::ptr::write_volatile(op_base as *mut u32, usbcmd | (1 << 1));
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    for i in 0..10000 {
        let cmd = core::ptr::read_volatile(op_base as *const u32);
        if cmd & (1 << 1) == 0 {
            crate::printk!("[XHCI] HCRST complete ({} iterations)", i);
            return true;
        }
        core::hint::spin_loop();
    }
    crate::printk!("[XHCI] WARNING: HCRST did not complete");
    false
}

/// Discover BIOS-configured event ring from Interrupter 0 registers.
/// Returns true if ERSTBA and ERDP are valid (BIOS configured the event ring).
/// Populates ctrl fields: evt_ring_phys, evt_ring_virt, evt_deq_idx, evt_cycle.
unsafe fn xhci_discover_existing_rings(ctrl: &mut XhciController) -> bool {
    let rts_base = ctrl.rts_base;

    // Read ERSTBA and ERDP from Interrupter 0 (standard offsets)
    let erstba = core::ptr::read_volatile(rts_base.add(0x10) as *const u64);
    let erdp = core::ptr::read_volatile(rts_base.add(0x18) as *const u64);

    crate::printk!("[XHCI] BIOS Intr0: ERSTBA={:#x} ERDP={:#x}", erstba, erdp);

    if erstba == 0 {
        crate::printk!("[XHCI] No BIOS event ring (ERSTBA=0)");
        return false;
    }

    // Read ERST entry to get event ring physical address
    let erst_virt = crate::memory::vmm::phys_to_virt(erstba) as *mut u8;
    let evt_ring_phys = core::ptr::read_volatile(erst_virt as *const u64);
    if evt_ring_phys == 0 {
        crate::printk!("[XHCI] ERST entry has no event ring address");
        return false;
    }
    let evt_ring_virt = crate::memory::vmm::phys_to_virt(evt_ring_phys) as *mut u8;

    crate::printk!(
        "[XHCI] BIOS event ring: phys={:#x} erst={:#x}",
        evt_ring_phys,
        erstba
    );

    // Populate controller struct
    ctrl.evt_ring_phys = evt_ring_phys;
    ctrl.evt_ring_virt = evt_ring_virt;

    // Derive dequeue index and cycle from ERDP
    let erdp_addr = erdp & !0xF;
    let erdp_dcs = erdp & 1;
    let evt_deq_idx = ((erdp_addr - evt_ring_phys) / 16) as u32;
    ctrl.evt_deq_idx = evt_deq_idx;
    ctrl.evt_cycle = erdp_dcs as u32;

    crate::printk!(
        "[XHCI] Evt ring: ERDP addr={:#x} DCS={} deq_idx={}",
        erdp_addr,
        erdp_dcs,
        evt_deq_idx
    );

    // Also save ERST physical address for later use
    ctrl.erst_phys = erstba;

    true
}

/// Initialize the xHCI controller after probe.
/// Strategy: first try to adopt BIOS-configured rings, then fall back to HCRST.
unsafe fn xhci_init_controller(ctrl: &mut XhciController) -> bool {
    let mmio = ctrl.mmio;
    let caplength = ctrl.caplength as usize;
    let op_base = mmio.add(caplength);

    // Read capability registers for initialization
    let hcs_params1 = core::ptr::read_volatile(mmio.add(4) as *const u32);
    let max_slots = (hcs_params1 >> 24) & 0xFF;
    let hcc_params1 = core::ptr::read_volatile(mmio.add(0x10) as *const u32);
    let _context_size_64 = (hcc_params1 & 2) != 0;
    let db_off = core::ptr::read_volatile(mmio.add(0x14) as *const u32);
    let rts_off = core::ptr::read_volatile(mmio.add(0x18) as *const u32);

    ctrl.max_slots = max_slots;
    ctrl.op_base = op_base;
    ctrl.db_base = mmio.add((db_off & !1) as usize);
    ctrl.rts_base = mmio.add(rts_off as usize);

    crate::printk!(
        "[XHCI] Init: max_slots={}, ctx64={}, db_off={:#x}, rts_off={:#x}",
        max_slots,
        _context_size_64,
        db_off,
        rts_off
    );

    // Detect register layout (QEMU vs standard)
    let (crcr_off, dcbaap_off, config_off) = detect_xhci_reg_layout(op_base);
    ctrl.crcr_off = crcr_off;
    ctrl.dcbaap_off = dcbaap_off;
    ctrl.config_off = config_off;
    crate::printk!(
        "[XHCI] Using offsets: CRCR=op+{:#x} DCBAAP=op+{:#x} CONFIG=op+{:#x}",
        crcr_off,
        dcbaap_off,
        config_off
    );

    // Halt the controller before any register access
    halt_xhci_controller(op_base);

    // Try to adopt BIOS-configured event ring first (avoids QEMU runtime register write issues)
    // Save discovered values temporarily to protect BIOS pages from xhci_free_dma
    let mut bio_evt_ring_phys = 0u64;
    let mut bio_evt_ring_virt: *mut u8 = core::ptr::null_mut();
    let mut bio_erst_phys = 0u64;
    let mut bio_evt_deq_idx = 0u32;
    let mut bio_evt_cycle = 0u32;
    let discovered = xhci_discover_existing_rings(ctrl);

    if discovered {
        // Extract BIOS values, then clear ctrl fields so xhci_free_dma won't touch BIOS pages
        bio_evt_ring_phys = ctrl.evt_ring_phys;
        bio_evt_ring_virt = ctrl.evt_ring_virt;
        bio_erst_phys = ctrl.erst_phys;
        bio_evt_deq_idx = ctrl.evt_deq_idx;
        bio_evt_cycle = ctrl.evt_cycle;
        ctrl.evt_ring_phys = 0;
        ctrl.evt_ring_virt = core::ptr::null_mut();
        ctrl.erst_phys = 0;

        crate::printk!("[XHCI] Adopting BIOS rings — no HCRST, own cmd ring + DCBAA");
        crate::printk!(
            "[XHCI] BIOS evt ring phys={:#x} deq_idx={} cycle={} erst={:#x}",
            bio_evt_ring_phys,
            bio_evt_deq_idx,
            bio_evt_cycle,
            bio_erst_phys
        );

        // Stop existing command ring before reconfiguring
        stop_xhci_command_ring(op_base, crcr_off);

        // Allocate command ring
        let cmd_ring_phys = match crate::memory::pmm::alloc_frames(1) {
            Some(p) => p,
            None => {
                crate::printk!("[XHCI] Failed to allocate command ring");
                return false;
            }
        };
        let cmd_ring_virt = crate::memory::vmm::phys_to_virt(cmd_ring_phys) as *mut u8;
        core::ptr::write_bytes(cmd_ring_virt, 0, 4096);
        ctrl.cmd_ring_phys = cmd_ring_phys;
        ctrl.cmd_ring_virt = cmd_ring_virt;
        ctrl.cmd_enq_idx = 0;
        ctrl.cmd_cycle = 1;

        // Link TRB at index 255
        let link_trb = cmd_ring_virt.add(255 * 16) as *mut u32;
        core::ptr::write_volatile(link_trb.add(0), cmd_ring_phys as u32);
        core::ptr::write_volatile(link_trb.add(1), (cmd_ring_phys >> 32) as u32);
        core::ptr::write_volatile(link_trb.add(2), 0);
        core::ptr::write_volatile(
            link_trb.add(3),
            (TRB_TYPE_LINK << TRB_TYPE_SHIFT) | TRB_TC | TRB_CYCLE,
        );
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        // Write CRCR at detected offset
        core::ptr::write_volatile(op_base.add(crcr_off) as *mut u32, cmd_ring_phys as u32 | 1);
        core::ptr::write_volatile(
            op_base.add(crcr_off + 4) as *mut u32,
            (cmd_ring_phys >> 32) as u32,
        );
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let crcr_lo = core::ptr::read_volatile(op_base.add(crcr_off) as *const u32);
        let crcr_hi = core::ptr::read_volatile(op_base.add(crcr_off + 4) as *const u32);
        crate::printk!("[XHCI] CRCR={:#x}{:#x}", crcr_hi, crcr_lo);

        // Allocate DCBAA
        let dcbaa_phys = match crate::memory::pmm::alloc_frames(1) {
            Some(p) => p,
            None => {
                crate::printk!("[XHCI] Failed to allocate DCBAA");
                xhci_free_dma(ctrl);
                return false;
            }
        };
        let dcbaa_virt = crate::memory::vmm::phys_to_virt(dcbaa_phys) as *mut u64;
        core::ptr::write_bytes(dcbaa_virt as *mut u8, 0, 4096);
        ctrl.dcbaa_phys = dcbaa_phys;
        ctrl.dcbaa_virt = dcbaa_virt;

        // Write DCBAAP at detected offset
        core::ptr::write_volatile(op_base.add(dcbaap_off) as *mut u64, dcbaa_phys);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let dcbaap_lo = core::ptr::read_volatile(op_base.add(dcbaap_off) as *const u32);
        let dcbaap_hi = core::ptr::read_volatile(op_base.add(dcbaap_off + 4) as *const u32);
        crate::printk!("[XHCI] DCBAAP={:#x}{:#x}", dcbaap_hi, dcbaap_lo);

        // Write CONFIG
        core::ptr::write_volatile(op_base.add(config_off) as *mut u32, max_slots & 0xFF);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let config = core::ptr::read_volatile(op_base.add(config_off) as *const u32);
        crate::printk!("[XHCI] CONFIG={:#x} (MaxSlotsEn={})", config, max_slots);

        // Restore BIOS event ring state now that allocations are done
        ctrl.evt_ring_phys = bio_evt_ring_phys;
        ctrl.evt_ring_virt = bio_evt_ring_virt;
        ctrl.erst_phys = bio_erst_phys;
        ctrl.evt_deq_idx = bio_evt_deq_idx;
        ctrl.evt_cycle = bio_evt_cycle;

        // Configure Interrupter 1 to point to the BIOS event ring.
        // All our TRBs set IT=1, so events route to Interrupter 1.
        // ERDP writes go to Interrupter 1 (shifted offset 0x38).
        ctrl.erdp_off = 0x38;
        {
            let rts_base = ctrl.rts_base;
            crate::printk!("[XHCI] Configuring Interrupter 1 for BIOS event ring...");
            core::ptr::write_volatile(rts_base.add(0x20) as *mut u32, 3);
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            core::ptr::write_volatile(rts_base.add(0x28) as *mut u32, 1);
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            core::ptr::write_volatile(rts_base.add(0x30) as *mut u32, bio_erst_phys as u32);
            core::ptr::write_volatile(rts_base.add(0x34) as *mut u32, (bio_erst_phys >> 32) as u32);
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            core::ptr::write_volatile(
                rts_base.add(0x38) as *mut u32,
                (bio_evt_ring_phys as u32) | 1,
            );
            core::ptr::write_volatile(
                rts_base.add(0x3C) as *mut u32,
                (bio_evt_ring_phys >> 32) as u32,
            );
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }

        // Start controller
        let usbcmd = core::ptr::read_volatile(op_base as *const u32);
        core::ptr::write_volatile(op_base as *mut u32, usbcmd | 1);
        for i in 0..10000 {
            let usbsts = core::ptr::read_volatile(op_base.add(4) as *const u32);
            if (usbsts & 1) == 0 {
                crate::printk!(
                    "[XHCI] Controller started ({} iter) USBSTS={:#x}",
                    i,
                    usbsts
                );
                // Verify BIOS event ring still intact after start
                let erstba = core::ptr::read_volatile(ctrl.rts_base.add(0x10) as *const u64);
                let erdp = core::ptr::read_volatile(ctrl.rts_base.add(0x18) as *const u64);
                crate::printk!("[XHCI] Post-start: ERSTBA={:#x} ERDP={:#x}", erstba, erdp);
                ctrl.initialized = true;
                return true;
            }
            core::hint::spin_loop();
        }
        crate::printk!("[XHCI] Controller failed to start after adopt");
        xhci_free_dma(ctrl);
        return false;
    }

    // ── No BIOS rings found — full HCRST + own allocation ──
    crate::printk!("[XHCI] No BIOS rings — doing HCRST + full init");
    stop_xhci_command_ring(op_base, crcr_off);

    if !xhci_hcrst(op_base) {
        crate::printk!("[XHCI] HCRST failed, attempting init anyway");
    }
    halt_xhci_controller(op_base);

    // 1. Allocate and initialize Command Ring
    let cmd_ring_phys = match crate::memory::pmm::alloc_frames(1) {
        Some(p) => p,
        None => {
            crate::printk!("[XHCI] Failed to allocate command ring");
            return false;
        }
    };
    let cmd_ring_virt = crate::memory::vmm::phys_to_virt(cmd_ring_phys) as *mut u8;
    core::ptr::write_bytes(cmd_ring_virt, 0, 4096);
    ctrl.cmd_ring_phys = cmd_ring_phys;
    ctrl.cmd_ring_virt = cmd_ring_virt;
    ctrl.cmd_enq_idx = 0;
    ctrl.cmd_cycle = 1;

    let link_trb = cmd_ring_virt.add(255 * 16) as *mut u32;
    core::ptr::write_volatile(link_trb.add(0), cmd_ring_phys as u32);
    core::ptr::write_volatile(link_trb.add(1), (cmd_ring_phys >> 32) as u32);
    core::ptr::write_volatile(link_trb.add(2), 0);
    core::ptr::write_volatile(
        link_trb.add(3),
        (TRB_TYPE_LINK << TRB_TYPE_SHIFT) | TRB_TC | TRB_CYCLE,
    );

    core::ptr::write_volatile(op_base.add(crcr_off) as *mut u32, cmd_ring_phys as u32 | 1);
    core::ptr::write_volatile(
        op_base.add(crcr_off + 4) as *mut u32,
        (cmd_ring_phys >> 32) as u32,
    );
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let crcr_lo = core::ptr::read_volatile(op_base.add(crcr_off) as *const u32);
    let crcr_hi = core::ptr::read_volatile(op_base.add(crcr_off + 4) as *const u32);
    crate::printk!("[XHCI] CRCR={:#x}{:#x}", crcr_hi, crcr_lo);

    // ERDP writes go to Interrupter 1 (shifted offset 0x38)
    ctrl.erdp_off = 0x38;

    // 2. Allocate Event Ring + ERST
    let evt_ring_phys = match crate::memory::pmm::alloc_frames(1) {
        Some(p) => p,
        None => {
            crate::printk!("[XHCI] Failed to allocate event ring");
            xhci_free_dma(ctrl);
            return false;
        }
    };
    let evt_ring_virt = crate::memory::vmm::phys_to_virt(evt_ring_phys) as *mut u8;
    core::ptr::write_bytes(evt_ring_virt, 0, 4096);
    ctrl.evt_ring_phys = evt_ring_phys;
    ctrl.evt_ring_virt = evt_ring_virt;
    ctrl.evt_deq_idx = 0;
    ctrl.evt_cycle = 1;

    let erst_phys = match crate::memory::pmm::alloc_frames(1) {
        Some(p) => p,
        None => {
            crate::printk!("[XHCI] Failed to allocate ERST");
            xhci_free_dma(ctrl);
            return false;
        }
    };
    let erst_virt = crate::memory::vmm::phys_to_virt(erst_phys) as *mut u8;
    core::ptr::write_bytes(erst_virt, 0, 4096);
    ctrl.erst_phys = erst_phys;

    core::ptr::write_volatile(erst_virt as *mut u64, evt_ring_phys);
    core::ptr::write_volatile(erst_virt.add(8) as *mut u32, 256);
    core::ptr::write_volatile(erst_virt.add(12) as *mut u32, 0);
    xhci_flush_range(erst_virt, 16);
    xhci_flush_range(evt_ring_virt, 4096); // ensure all 0-init event ring is visible to xHC

    // 3. Allocate DCBAA
    let dcbaa_phys = match crate::memory::pmm::alloc_frames(1) {
        Some(p) => p,
        None => {
            crate::printk!("[XHCI] Failed to allocate DCBAA");
            xhci_free_dma(ctrl);
            return false;
        }
    };
    let dcbaa_virt = crate::memory::vmm::phys_to_virt(dcbaa_phys) as *mut u64;
    core::ptr::write_bytes(dcbaa_virt as *mut u8, 0, 4096);
    xhci_flush_range(dcbaa_virt as *mut u8, 4096);
    ctrl.dcbaa_phys = dcbaa_phys;
    ctrl.dcbaa_virt = dcbaa_virt;

    core::ptr::write_volatile(op_base.add(dcbaap_off) as *mut u64, dcbaa_phys);
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let dcbaap_lo = core::ptr::read_volatile(op_base.add(dcbaap_off) as *const u32);
    let dcbaap_hi = core::ptr::read_volatile(op_base.add(dcbaap_off + 4) as *const u32);
    crate::printk!("[XHCI] DCBAAP={:#x}{:#x}", dcbaap_hi, dcbaap_lo);

    // 4. Write CONFIG
    core::ptr::write_volatile(op_base.add(config_off) as *mut u32, max_slots & 0xFF);
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    let config = core::ptr::read_volatile(op_base.add(config_off) as *const u32);
    crate::printk!("[XHCI] CONFIG={:#x} (MaxSlotsEn={})", config, max_slots);

    // 5. Write runtime registers at QEMU-accepted shifted offsets (Interrupter 1)
    {
        let rts_base = ctrl.rts_base;
        crate::printk!("[XHCI] Setting up Interrupter 1 (shifted offsets)...");

        core::ptr::write_volatile(rts_base.add(0x20) as *mut u32, 3);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        core::ptr::write_volatile(rts_base.add(0x28) as *mut u32, 1);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        core::ptr::write_volatile(rts_base.add(0x30) as *mut u32, erst_phys as u32);
        core::ptr::write_volatile(rts_base.add(0x34) as *mut u32, (erst_phys >> 32) as u32);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        core::ptr::write_volatile(rts_base.add(0x38) as *mut u32, (evt_ring_phys as u32) | 1);
        core::ptr::write_volatile(rts_base.add(0x3C) as *mut u32, (evt_ring_phys >> 32) as u32);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        let iman = core::ptr::read_volatile(rts_base.add(0x20) as *const u32);
        let erstsz = core::ptr::read_volatile(rts_base.add(0x28) as *const u32);
        let erstba = core::ptr::read_volatile(rts_base.add(0x30) as *const u32);
        let erdp = core::ptr::read_volatile(rts_base.add(0x38) as *const u32);
        crate::printk!(
            "[XHCI] Intr1: IMAN={:#x} ERSTSZ={:#x} ERSTBA={:#x} ERDP={:#x}",
            iman,
            erstsz,
            erstba,
            erdp
        );
    }

    // 6. Start controller
    let usbcmd = core::ptr::read_volatile(op_base as *const u32);
    core::ptr::write_volatile(op_base as *mut u32, usbcmd | 1);

    for i in 0..10000 {
        let usbsts = core::ptr::read_volatile(op_base.add(4) as *const u32);
        if (usbsts & 1) == 0 {
            crate::printk!(
                "[XHCI] Controller started ({} iter) USBSTS={:#x}",
                i,
                usbsts
            );

            // Retry runtime writes after start
            let rts_base = ctrl.rts_base;
            crate::printk!("[XHCI] Retrying Interrupter 1 writes after start...");
            core::ptr::write_volatile(rts_base.add(0x20) as *mut u32, 3);
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            core::ptr::write_volatile(rts_base.add(0x28) as *mut u32, 1);
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            core::ptr::write_volatile(rts_base.add(0x30) as *mut u32, ctrl.erst_phys as u32);
            core::ptr::write_volatile(
                rts_base.add(0x34) as *mut u32,
                (ctrl.erst_phys >> 32) as u32,
            );
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            core::ptr::write_volatile(
                rts_base.add(0x38) as *mut u32,
                (ctrl.evt_ring_phys as u32) | 1,
            );
            core::ptr::write_volatile(
                rts_base.add(0x3C) as *mut u32,
                (ctrl.evt_ring_phys >> 32) as u32,
            );
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

            let i2 = core::ptr::read_volatile(rts_base.add(0x20) as *const u32);
            let e2 = core::ptr::read_volatile(rts_base.add(0x28) as *const u32);
            let b2 = core::ptr::read_volatile(rts_base.add(0x30) as *const u32);
            let d2 = core::ptr::read_volatile(rts_base.add(0x38) as *const u32);
            crate::printk!(
                "[XHCI] Post-start Intr1: IMAN={:#x} ERSTSZ={:#x} ERSTBA={:#x} ERDP={:#x}",
                i2,
                e2,
                b2,
                d2
            );

            // Dump operational registers
            let op_usbcmd = core::ptr::read_volatile(op_base as *const u32);
            let op_usbsts = core::ptr::read_volatile(op_base.add(4) as *const u32);
            let op_crcr_lo = core::ptr::read_volatile(op_base.add(crcr_off) as *const u32);
            let op_crcr_hi = core::ptr::read_volatile(op_base.add(crcr_off + 4) as *const u32);
            let op_dcbaap_lo = core::ptr::read_volatile(op_base.add(dcbaap_off) as *const u32);
            let op_dcbaap_hi = core::ptr::read_volatile(op_base.add(dcbaap_off + 4) as *const u32);
            let op_config = core::ptr::read_volatile(op_base.add(config_off) as *const u32);
            crate::printk!(
                "[XHCI] OP regs: USBCMD={:#x} USBSTS={:#x} PAGESIZE={:#x} DNCTRL={:#x}",
                op_usbcmd,
                op_usbsts,
                core::ptr::read_volatile(op_base.add(8) as *const u32),
                core::ptr::read_volatile(op_base.add(0xc) as *const u32),
            );
            crate::printk!(
                "[XHCI] OP regs: CRCR={:#x}{:#x} DCBAAP={:#x}{:#x} CONFIG={:#x}",
                op_crcr_hi,
                op_crcr_lo,
                op_dcbaap_hi,
                op_dcbaap_lo,
                op_config
            );

            ctrl.initialized = true;
            return true;
        }
        core::hint::spin_loop();
    }

    crate::printk!("[XHCI] Controller failed to start (HCHalted still set)");
    xhci_free_dma(ctrl);
    false
}

/// Kernel task: periodically poll xHCI controllers for port status changes.
pub fn xhci_poll_task_fn() {
    crate::printk!("[XHCI] Polling task started");
    loop {
        poll_xhci_controllers();
        crate::scheduler::sleep(50); // ~500ms at ~100Hz
    }
}

/// Spawn the xHCI polling kernel task. Safe to call after scheduler::init().
pub fn start_xhci_polling() {
    crate::scheduler::spawn("xhci_poll", xhci_poll_task_fn);
    crate::printk!("[XHCI] Polling task spawned");
}

// ── Batch 8: xhci-pci module support ─────────────────────

#[no_mangle]
pub extern "C" fn __shim_xhci_pci_common_probe(dev: *mut u8, _id: *mut u8) -> *mut u8 {
    crate::printk!("[SHIM] xhci_pci_common_probe: called");
    if dev.is_null() {
        return (-19isize) as *mut u8;
    }

    let (bus, pci_device, function) = pci_dev_to_bdf(dev);

    // Check if this BDF already has an initialized controller.
    // If so, skip the heavy init (HCRST would destroy our event ring state).
    // Use try_lock to avoid deadlock with poll task.
    let hcd_to_return = {
        let controllers = loop {
            match XHCI_CONTROLLERS.try_lock() {
                Some(c) => break c,
                None => crate::scheduler::yield_now(),
            }
        };
        controllers.iter().find_map(|c| {
            if c.bus == bus && c.device == pci_device && c.function == function && c.initialized {
                Some(c.hcd)
            } else {
                None
            }
        })
    };
    if let Some(hcd) = hcd_to_return {
        crate::printk!(
            "[SHIM] xhci_pci: BDF {}.{}.{} already initialized, skipping re-probe",
            bus,
            pci_device,
            function
        );
        return hcd;
    }

    // Invalidate any existing (stale) controller for this BFD before we reinit.
    {
        let mut controllers = loop {
            match XHCI_CONTROLLERS.try_lock() {
                Some(c) => break c,
                None => crate::scheduler::yield_now(),
            }
        };
        for existing in controllers.iter_mut() {
            if existing.bus == bus && existing.device == pci_device && existing.function == function
            {
                existing.initialized = false;
                crate::printk!(
                    "[SHIM] xhci_pci: invalidated stale ctrl_idx={} for BDF {}.{}.{}",
                    existing.ctrl_idx,
                    bus,
                    pci_device,
                    function
                );
            }
        }
    }

    // Enable PCI device and set bus master
    let ret = __shim_pci_enable_device(dev);
    if ret != 0 {
        crate::printk!("[SHIM] xhci_pci: pci_enable_device failed: {}", ret);
        return (ret as isize) as *mut u8;
    }
    __shim_pci_set_master(dev);

    // Read MMIO BAR (BAR0)
    let bar_addr = __shim_pci_resource_start(dev, 0);
    let bar_len = __shim_pci_resource_len(dev, 0);
    crate::printk!(
        "[SHIM] xhci_pci: BAR0 addr={:#x} len={:#x}",
        bar_addr,
        bar_len
    );

    if bar_addr == 0 || bar_len == 0 {
        crate::printk!("[SHIM] xhci_pci: no MMIO BAR found");
        return (-19isize) as *mut u8;
    }

    // Map MMIO
    let mmio = __shim_pci_iomap(dev, 0, 0);
    if mmio.is_null() {
        crate::printk!("[SHIM] xhci_pci: iomap failed");
        return (-12isize) as *mut u8;
    }
    crate::printk!("[SHIM] xhci_pci: MMIO mapped at {:p}", mmio);

    // Read xHCI capability registers
    // Capability Registers (at MMIO base):
    //   0x00: CAPLENGTH  (1 byte) — length of cap regs, also offset to operational regs
    //   0x02: HCIVERSION (2 bytes)
    //   0x04: HCSPARAMS1 (4 bytes) — bits [7:0] = MaxPorts
    //   0x10: HCCPARAMS1 (4 bytes)
    let (caplength, hciver, max_ports, hcc_params) = unsafe {
        let caplength = core::ptr::read_volatile(mmio as *const u8);
        let hciver = core::ptr::read_volatile(mmio.add(2) as *const u16);
        let hcs_params = core::ptr::read_volatile(mmio.add(4) as *const u32);
        let max_ports = (hcs_params & 0xFF) as u32;
        let hcc_params = core::ptr::read_volatile(mmio.add(0x10) as *const u32);
        (caplength, hciver, max_ports, hcc_params)
    };

    crate::printk!(
        "[SHIM] xhci_pci: CAPLENGTH={}, HCIVERSION={:#x}",
        caplength,
        hciver
    );
    crate::printk!(
        "[SHIM] xhci_pci: HCSPARAMS1={:#x}, max_ports={}",
        unsafe { core::ptr::read_volatile(mmio.add(4) as *const u32) },
        max_ports
    );
    crate::printk!("[SHIM] xhci_pci: HCCPARAMS1={:#x}", hcc_params);

    // Create HCD via our stub
    let hcd = __shim_usb_create_hcd(core::ptr::null(), dev, b"xhci\0".as_ptr());
    if hcd.is_null() {
        crate::printk!("[SHIM] xhci_pci: usb_create_hcd failed");
        return (-12isize) as *mut u8;
    }

    let add_ret = __shim_usb_add_hcd(hcd, 0, 0);
    if add_ret != 0 {
        crate::printk!("[SHIM] xhci_pci: usb_add_hcd failed: {}", add_ret);
        return (add_ret as isize) as *mut u8;
    }

    // Track this controller for root hub polling
    let mut ctrl = XhciController {
        mmio,
        op_base: core::ptr::null_mut(),
        db_base: core::ptr::null_mut(),
        rts_base: core::ptr::null_mut(),
        caplength,
        max_ports,
        max_slots: 0,
        hcd,
        bus,
        device: pci_device,
        function,
        initialized: false,
        ctrl_idx: 0,
        crcr_off: 0x18,
        dcbaap_off: 0x30,
        config_off: 0x38,
        cmd_ring_phys: 0,
        cmd_ring_virt: core::ptr::null_mut(),
        cmd_enq_idx: 0,
        cmd_cycle: 0,
        evt_ring_phys: 0,
        evt_ring_virt: core::ptr::null_mut(),
        evt_deq_idx: 0,
        evt_cycle: 1, // xHC writes events with cycle=1 after HCRST (DCS=1 in ERDP)
        erst_phys: 0,
        dcbaa_phys: 0,
        dcbaa_virt: core::ptr::null_mut(),
        port_slot: [0u8; 256],
        slot_port: [0u8; 32],
        num_devices: 0,
        ep0_ring_phys: 0,
        ep0_ring_virt: core::ptr::null_mut(),
        ep0_trb_idx: 0,
        ep0_cycle: 1,
        ep_out_ring: [EndpointRing::empty(); 32],
        ep_in_ring: [EndpointRing::empty(); 32],
        bulk_ep_configured: [false; 32],
        evt_mismatch_count: 0,
        erdp_off: 0x18,
        enum_debug: EnumDebugInfo::new(),
    };

    // Initialize the xHCI controller
    let init_ok = unsafe { xhci_init_controller(&mut ctrl) };
    if !init_ok {
        crate::printk!("[SHIM] xhci_pci: controller init failed");
    }

    // Use try_lock + yield to avoid deadlock with poll task (spin::Mutex + preemption)
    let mut controllers;
    loop {
        match XHCI_CONTROLLERS.try_lock() {
            Some(c) => {
                controllers = c;
                break;
            }
            None => crate::scheduler::yield_now(),
        }
    }
    ctrl.ctrl_idx = controllers.len();
    controllers.push(ctrl);

    crate::printk!("[SHIM] xhci_pci: probe SUCCESS (hcd at {:p})", hcd);
    hcd
}

// ── USB Mass Storage Class Driver (Bulk-Only Transport) ─────
// Implements the USB BOT protocol directly using xHCI bulk transfers,
// without requiring the Linux SCSI midlayer.

/// Send a CBW (Command Block Wrapper) on bulk OUT endpoint (EP1 OUT = epid 2)
unsafe fn bot_send_cbw(
    ctrl: &mut XhciController,
    slot_id: u32,
    tag: u32,
    data_len: u32,
    dir_in: bool,
    lun: u8,
    cdb: &[u8],
) -> bool {
    let mut cbw = [0u8; 31];
    cbw[0..4].copy_from_slice(&CBW_SIGNATURE.to_le_bytes());
    cbw[4..8].copy_from_slice(&tag.to_le_bytes());
    cbw[8..12].copy_from_slice(&data_len.to_le_bytes());
    cbw[12] = if dir_in { 0x80 } else { 0x00 };
    cbw[13] = lun;
    cbw[14] = cdb.len() as u8;
    let copy_len = core::cmp::min(cdb.len(), 16);
    cbw[15..15 + copy_len].copy_from_slice(&cdb[..copy_len]);

    xhci_bulk_transfer(ctrl, slot_id, 2, cbw.as_mut_ptr(), 31)
}

/// Read a CSW (Command Status Wrapper) from bulk IN endpoint (EP1 IN = epid 3)
unsafe fn bot_read_csw(ctrl: &mut XhciController, slot_id: u32, tag: u32) -> Option<u8> {
    let mut csw = [0u8; 13];
    if !xhci_bulk_transfer(ctrl, slot_id, 3, csw.as_mut_ptr(), 13) {
        return None;
    }
    let sig = u32::from_le_bytes([csw[0], csw[1], csw[2], csw[3]]);
    if sig != CSW_SIGNATURE {
        crate::printk!("[USBSTOR] Bad CSW signature: {:#x}", sig);
        return None;
    }
    let csw_tag = u32::from_le_bytes([csw[4], csw[5], csw[6], csw[7]]);
    if csw_tag != tag {
        crate::printk!("[USBSTOR] CSW tag mismatch: {} != {}", csw_tag, tag);
        return None;
    }
    Some(csw[12]) // 0=pass, 1=fail, 2=phase error
}

/// Execute a full BOT command: CBW → Data (optional) → CSW
unsafe fn bot_transfer(
    ctrl: &mut XhciController,
    slot_id: u32,
    tag: u32,
    cdb: &[u8],
    data_buf: *mut u8,
    data_len: u32,
    dir_in: bool,
) -> bool {
    if !bot_send_cbw(ctrl, slot_id, tag, data_len, dir_in, 0, cdb) {
        crate::printk!("[USBSTOR] CBW send failed");
        return false;
    }
    if data_len > 0 {
        let ep = if dir_in { 3 } else { 2 };
        if !xhci_bulk_transfer(ctrl, slot_id, ep, data_buf, data_len) {
            crate::printk!("[USBSTOR] Data phase failed");
            return false;
        }
    }
    match bot_read_csw(ctrl, slot_id, tag) {
        Some(0) => true,
        Some(status) => {
            crate::printk!("[USBSTOR] CSW status={}", status);
            false
        }
        None => false,
    }
}

/// Probe a USB mass storage device and try to read its capacity + first sector
/// Dump USB/xHCI status for all controllers and devices
pub fn usb_status_dump() {
    use crate::mesa_println;
    let controllers = loop {
        match XHCI_CONTROLLERS.try_lock() {
            Some(c) => break c,
            None => crate::scheduler::yield_now(),
        }
    };
    let records = loop {
        match USB_DEVICE_RECORDS.try_lock() {
            Some(r) => break r,
            None => crate::scheduler::yield_now(),
        }
    };

    mesa_println!("=== USB / xHCI Status ===");
    mesa_println!("Controllers: {}", controllers.len());
    mesa_println!("Devices: {}", records.len());
    mesa_println!();

    for (ci, ctrl) in controllers.iter().enumerate() {
        let mmio_valid = if ctrl.mmio.is_null() { "no" } else { "yes" };
        let init_state = if ctrl.initialized { "YES" } else { "NO" };
        mesa_println!("--- Controller {} ---", ci);
        mesa_println!(
            "  MMIO: {} | Init: {} | BDF: {}.{}.{}",
            mmio_valid,
            init_state,
            ctrl.bus,
            ctrl.device,
            ctrl.function
        );
        mesa_println!(
            "  HCIVersion: ... | MaxPorts: {} | MaxSlots: {}",
            ctrl.max_ports,
            ctrl.max_slots
        );
        if ctrl.initialized {
            unsafe {
                let usbsts = core::ptr::read_volatile(ctrl.op_base.add(4) as *const u32);
                mesa_println!(
                    "  USBSTS: {:#010x} (HCHalted={}, HostSysErr={})",
                    usbsts,
                    (usbsts >> 0) & 1,
                    (usbsts >> 4) & 1,
                );
            }
        }
        mesa_println!("  NumDevices: {}", ctrl.num_devices);
        for port in 0..ctrl.max_ports {
            if ctrl.port_slot[port as usize] != 0 {
                mesa_println!("  Port {}: slot={}", port, ctrl.port_slot[port as usize]);
            }
        }
        // Enumeration debug
        let ed = &ctrl.enum_debug;
        let stage_name = ed.stage.name();
        let ticks = crate::curr_arch::get_ticks();
        let elapsed = if ed.tick_start != 0 {
            ticks.wrapping_sub(ed.tick_start)
        } else {
            0
        };
        let stage_elapsed = if ed.tick_stage != 0 {
            ticks.wrapping_sub(ed.tick_stage)
        } else {
            0
        };
        mesa_println!(
            "  EnumStage: {} (total={} ticks, in_stage={})",
            stage_name,
            elapsed,
            stage_elapsed
        );
        if ed.stage == EnumStage::Error {
            mesa_println!("  ERROR: {}", ed.error_string());
            mesa_println!("  Retries: {}", ed.retry_count);
            if ed.timed_out {
                mesa_println!("  TIMED OUT");
            }
        }
        mesa_println!();
    }

    // Dump device records
    if !records.is_empty() {
        mesa_println!("--- USB Device Records ---");
        for (addr, rec) in records.iter() {
            mesa_println!("  addr={:#010x} ctrl={} slot={} port={} speed={} vid={:#04x} pid={:#04x} class={:#x}",
                addr, rec.ctrl_idx, rec.slot_id, rec.port, rec.speed,
                rec.vendor_id, rec.product_id, rec.device_class);
        }
        mesa_println!();
    }

    mesa_println!("=== End USB Status ===");
}

pub unsafe fn usb_storage_probe(ctrl: &mut XhciController, slot_id: u32, port: u32) {
    crate::printk!(
        "[USBSTOR] Probing mass storage device slot={} port={}",
        slot_id,
        port
    );

    // 1. Read full config descriptor (up to 512 bytes)
    let mut cfg_desc = [0u8; 512];
    let setup_cfg = [
        USB_DIR_IN,
        USB_REQ_GET_DESCRIPTOR,
        0,
        USB_DT_CONFIG,
        0,
        0,
        core::mem::size_of::<[u8; 512]>() as u8,
        (core::mem::size_of::<[u8; 512]>() >> 8) as u8,
    ];
    // First read just the header to get total length
    let mut cfg_hdr = [0u8; 9];
    let setup_cfg_hdr = [
        USB_DIR_IN,
        USB_REQ_GET_DESCRIPTOR,
        0,
        USB_DT_CONFIG,
        0,
        0,
        9,
        0,
    ];
    if !xhci_control_transfer(ctrl, slot_id, &setup_cfg_hdr, cfg_hdr.as_mut_ptr(), 9, true) {
        crate::printk!("[USBSTOR] Failed to read config header");
        return;
    }
    let total_len = (cfg_hdr[7] as u16) << 8 | cfg_hdr[6] as u16;
    if total_len as usize > cfg_desc.len() {
        crate::printk!("[USBSTOR] Config too large: {} bytes", total_len);
        return;
    }
    // Read full config
    let setup_full = [
        USB_DIR_IN,
        USB_REQ_GET_DESCRIPTOR,
        0,
        USB_DT_CONFIG,
        0,
        0,
        total_len as u8,
        (total_len >> 8) as u8,
    ];
    if !xhci_control_transfer(
        ctrl,
        slot_id,
        &setup_full,
        cfg_desc.as_mut_ptr(),
        total_len,
        true,
    ) {
        crate::printk!("[USBSTOR] Failed to read full config descriptor");
        return;
    }
    crate::printk!("[USBSTOR] Config descriptor: total_len={}", total_len);

    // 2. Parse interfaces for mass storage class
    let mut found_ms = false;
    let mut off = 0usize;
    while off + 2 <= total_len as usize {
        let len = cfg_desc[off] as usize;
        let dtype = cfg_desc[off + 1];
        if len < 2 || off + len > total_len as usize {
            break;
        }
        if dtype == USB_DT_INTERFACE && len >= 9 {
            let if_class = cfg_desc[off + 5];
            let if_subclass = cfg_desc[off + 6];
            let if_proto = cfg_desc[off + 7];
            let num_eps = cfg_desc[off + 4];
            crate::printk!(
                "[USBSTOR] Interface: class={:#x} subclass={:#x} proto={:#x} eps={}",
                if_class,
                if_subclass,
                if_proto,
                num_eps
            );
            if if_class == USB_CLASS_MASS_STORAGE && if_proto == USB_PROTO_BULK_ONLY {
                found_ms = true;
                // Parse endpoints within this interface
                let mut ep_off = off + len;
                for _ep_idx in 0..num_eps {
                    if ep_off + 7 > total_len as usize {
                        break;
                    }
                    if cfg_desc[ep_off + 1] == USB_DT_ENDPOINT {
                        let ep_addr = cfg_desc[ep_off + 2];
                        let ep_attr = cfg_desc[ep_off + 3];
                        let dir_in = (ep_addr & USB_ENDPOINT_DIR_MASK) != 0;
                        let xfer_type = ep_attr & 0x03;
                        crate::printk!(
                            "[USBSTOR]   Endpoint: addr={:#x} {} {}",
                            ep_addr,
                            if dir_in { "IN" } else { "OUT" },
                            if xfer_type == USB_ENDPOINT_XFER_BULK {
                                "BULK"
                            } else {
                                "other"
                            }
                        );
                    }
                    ep_off += cfg_desc[ep_off] as usize;
                }
            }
        }
        off += len;
    }

    if !found_ms {
        crate::printk!("[USBSTOR] No mass storage interface found");
        return;
    }

    // 3. BOT protocol: send SCSI INQUIRY
    let mut tag = 1u32;
    let mut inquiry_buf = [0u8; 36];
    let inquiry_cdb = [SCSI_INQUIRY, 0, 0, 0, 36, 0];
    if !bot_transfer(
        ctrl,
        slot_id,
        tag,
        &inquiry_cdb,
        inquiry_buf.as_mut_ptr(),
        36,
        true,
    ) {
        crate::printk!("[USBSTOR] INQUIRY failed");
        return;
    }
    tag += 1;

    let vendor = core::str::from_utf8(&inquiry_buf[8..24]).unwrap_or("?");
    let product = core::str::from_utf8(&inquiry_buf[24..40]).unwrap_or("?");
    let rev = core::str::from_utf8(&inquiry_buf[40..44]).unwrap_or("?");
    crate::printk!(
        "[USBSTOR] INQUIRY: Vendor='{}' Product='{}' Rev='{}'",
        vendor.trim_end(),
        product.trim_end(),
        rev.trim_end()
    );

    // 4. READ CAPACITY 10
    let mut cap_buf = [0u8; 8];
    let capacity_cdb = [SCSI_READ_CAPACITY_10, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    if !bot_transfer(
        ctrl,
        slot_id,
        tag,
        &capacity_cdb,
        cap_buf.as_mut_ptr(),
        8,
        true,
    ) {
        crate::printk!("[USBSTOR] READ CAPACITY failed");
        return;
    }
    tag += 1;

    let last_lba = u32::from_be_bytes([cap_buf[0], cap_buf[1], cap_buf[2], cap_buf[3]]);
    let block_size = u32::from_be_bytes([cap_buf[4], cap_buf[5], cap_buf[6], cap_buf[7]]);
    let total_size = (last_lba as u64 + 1) * block_size as u64;
    crate::printk!(
        "[USBSTOR] Capacity: last_lba={} block_size={} total_size={} bytes ({} MB)",
        last_lba,
        block_size,
        total_size,
        total_size / (1024 * 1024)
    );

    // 5. Read first sector (LBA 0)
    let mut sector = [0u8; 512];
    let read_cdb = [
        SCSI_READ_10,
        0,
        0,
        0,
        0,
        0, // LBA = 0
        0,
        0,
        0,
        1, // transfer length = 1 block
    ];
    if !bot_transfer(
        ctrl,
        slot_id,
        tag,
        &read_cdb,
        sector.as_mut_ptr(),
        512,
        true,
    ) {
        crate::printk!("[USBSTOR] READ 10 (LBA=0) failed");
        return;
    }
    tag += 1;

    // Print first 64 bytes of sector 0 (MBR or partition table)
    crate::printk!(
        "[USBSTOR] Sector 0 (first 64 bytes): {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} ...",
        sector[0], sector[1], sector[2], sector[3],
        sector[4], sector[5], sector[6], sector[7]
    );
    // Check for MBR signature (0x55AA at bytes 510-511)
    let mbr_sig = (sector[511] as u16) << 8 | sector[510] as u16;
    crate::printk!(
        "[USBSTOR] MBR signature: {:#x} {}",
        mbr_sig,
        if mbr_sig == 0x55AA { "(valid MBR)" } else { "" }
    );

    crate::printk!("[USBSTOR] USB mass storage device initialized successfully!");
}

#[no_mangle]
pub extern "C" fn __shim_xhci_pci_remove(_dev: *mut u8) {
    crate::printk!("[SHIM] xhci_pci_remove: called (stub)");
}
