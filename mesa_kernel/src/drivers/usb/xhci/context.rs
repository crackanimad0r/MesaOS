use xhci::context::{Input64Byte, Input32Byte, EndpointType, InputHandler, DeviceHandler};

pub fn configure_input_context(
    in_ctx_virt: *mut u8,
    slot_id: u8,
    port_id: u8,
    speed: u8,
    ep0_dequeue: u64,
    csz: bool,
) {
    let max_packet_size = match speed {
        4 => 512, // SuperSpeed
        3 => 64,  // High Speed
        2 => 8,   // Low
        1 | _ => 64,
    };

    unsafe {
        if csz {
            let in_ctx = &mut *(in_ctx_virt as *mut Input64Byte);
            in_ctx.control_mut().set_add_context_flag(0); // Slot Context = bit 0
            in_ctx.control_mut().set_add_context_flag(1); // EP0 Context = bit 1

            let slot_ctx = in_ctx.device_mut().slot_mut();
            slot_ctx.set_root_hub_port_number(port_id + 1);
            slot_ctx.set_context_entries(1);
            slot_ctx.set_speed(speed);

            let ep0_ctx = in_ctx.device_mut().endpoint_mut(1);
            ep0_ctx.set_error_count(3);
            ep0_ctx.set_endpoint_type(EndpointType::Control);
            ep0_ctx.set_max_packet_size(max_packet_size);
            ep0_ctx.set_tr_dequeue_pointer(ep0_dequeue);
            ep0_ctx.set_dequeue_cycle_state();
            ep0_ctx.set_average_trb_length(8);
        } else {
            let in_ctx = &mut *(in_ctx_virt as *mut Input32Byte);
            in_ctx.control_mut().set_add_context_flag(0); // Slot Context = bit 0
            in_ctx.control_mut().set_add_context_flag(1); // EP0 Context = bit 1

            let slot_ctx = in_ctx.device_mut().slot_mut();
            slot_ctx.set_root_hub_port_number(port_id + 1);
            slot_ctx.set_context_entries(1);
            slot_ctx.set_speed(speed);

            let ep0_ctx = in_ctx.device_mut().endpoint_mut(1);
            ep0_ctx.set_error_count(3);
            ep0_ctx.set_endpoint_type(EndpointType::Control);
            ep0_ctx.set_max_packet_size(max_packet_size);
            ep0_ctx.set_tr_dequeue_pointer(ep0_dequeue);
            ep0_ctx.set_dequeue_cycle_state();
            ep0_ctx.set_average_trb_length(8);
        }
    }
}

pub fn add_endpoint_context(
    in_ctx_virt: *mut u8,
    dci: u8,
    ep_type: EndpointType,
    max_packet_size: u16,
    tr_dequeue: u64,
    csz: bool,
) {
    unsafe {
        if csz {
            let in_ctx = &mut *(in_ctx_virt as *mut Input64Byte);
            in_ctx.control_mut().set_add_context_flag(dci as usize);
            in_ctx.control_mut().set_add_context_flag(0); // Slot Context
            
            let slot_ctx = in_ctx.device_mut().slot_mut();
            if dci > slot_ctx.context_entries() {
                slot_ctx.set_context_entries(dci);
            }

            let ep_ctx = in_ctx.device_mut().endpoint_mut(dci as usize);
            ep_ctx.set_error_count(3);
            ep_ctx.set_endpoint_type(ep_type);
            ep_ctx.set_max_packet_size(max_packet_size);
            ep_ctx.set_tr_dequeue_pointer(tr_dequeue);
            ep_ctx.set_dequeue_cycle_state();
            ep_ctx.set_average_trb_length(8);
        } else {
            let in_ctx = &mut *(in_ctx_virt as *mut Input32Byte);
            in_ctx.control_mut().set_add_context_flag(dci as usize);
            in_ctx.control_mut().set_add_context_flag(0); // Slot Context
            
            let slot_ctx = in_ctx.device_mut().slot_mut();
            if dci > slot_ctx.context_entries() {
                slot_ctx.set_context_entries(dci);
            }

            let ep_ctx = in_ctx.device_mut().endpoint_mut(dci as usize);
            ep_ctx.set_error_count(3);
            ep_ctx.set_endpoint_type(ep_type);
            ep_ctx.set_max_packet_size(max_packet_size);
            ep_ctx.set_tr_dequeue_pointer(tr_dequeue);
            ep_ctx.set_dequeue_cycle_state();
            ep_ctx.set_average_trb_length(8);
        }
    }
}
