use super::loader;
use super::scm;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Debug, Clone)]
pub struct ShimInstance {
    pub id: u64,
    pub name: String,
    pub region_phys: u64,
    pub task_id: Option<u64>,
    pub state: ShimState,
    pub heartbeat: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShimState {
    Running,
    Crashed,
    Suspended,
    Dead,
}

static SHIM_INSTANCES: Mutex<Vec<ShimInstance>> = Mutex::new(Vec::new());
static NEXT_SHIM_ID: Mutex<u64> = Mutex::new(1);

pub unsafe fn create_shim(
    name: &str,
    _module_phys: u64,
    _module_size: u64,
) -> Result<u64, &'static str> {
    let id = {
        let mut next = NEXT_SHIM_ID.lock();
        let id = *next;
        *next += 1;
        id
    };

    let region_phys = crate::memory::pmm::alloc_frames(4).ok_or("No memory for shim region")?;
    let region_virt = crate::memory::vmm::phys_to_virt(region_phys);
    let region = &mut *(region_virt as *mut scm::ShimRegion);
    region.init();

    let instance = ShimInstance {
        id,
        name: name.to_string(),
        region_phys,
        task_id: None,
        state: ShimState::Running,
        heartbeat: 0,
    };

    SHIM_INSTANCES.lock().push(instance);

    crate::printk!("[SHIM] Created shim instance '{}' id={}", name, id);
    Ok(id)
}

pub fn destroy_shim(id: u64) -> Result<(), &'static str> {
    let mut instances = SHIM_INSTANCES.lock();
    if let Some(pos) = instances.iter().position(|i| i.id == id) {
        let instance = instances.remove(pos);
        let frames = 4;
        let page_size = crate::memory::PAGE_SIZE as u64;
        for i in 0..frames {
            crate::memory::pmm::free_frame(instance.region_phys + (i as u64) * page_size);
        }
        crate::printk!(
            "[SHIM] Destroyed shim instance '{}' id={}",
            instance.name,
            id
        );
        Ok(())
    } else {
        Err("Shim not found")
    }
}

pub fn find_shim(id: u64) -> Option<ShimInstance> {
    SHIM_INSTANCES.lock().iter().find(|i| i.id == id).cloned()
}

pub fn list_shims() -> Vec<ShimInstance> {
    SHIM_INSTANCES.lock().clone()
}

pub fn update_heartbeat(id: u64) {
    if let Some(instance) = SHIM_INSTANCES.lock().iter_mut().find(|i| i.id == id) {
        instance.heartbeat = crate::curr_arch::get_ticks();
    }
}

pub fn check_heartbeats() {
    let now = crate::curr_arch::get_ticks();
    let mut instances = SHIM_INSTANCES.lock();
    for instance in instances.iter_mut() {
        if instance.state == ShimState::Running {
            if now.wrapping_sub(instance.heartbeat) > 180 {
                instance.state = ShimState::Crashed;
                crate::printk!(
                    "[SHIM] Shim '{}' id={} heartbeat timeout - marking crashed",
                    instance.name,
                    instance.id
                );
            }
        }
    }
}

pub unsafe fn send_command(id: u64, cmd: &scm::ScmCommand) -> Result<(), &'static str> {
    let instances = SHIM_INSTANCES.lock();
    if let Some(instance) = instances.iter().find(|i| i.id == id) {
        let region_virt = crate::memory::vmm::phys_to_virt(instance.region_phys);
        let region = &*(region_virt as *const scm::ShimRegion);
        if scm::scm_queue_push(&region.cmd_queue, cmd) != 0 {
            return Err("Command queue full");
        }
        Ok(())
    } else {
        Err("Shim not found")
    }
}

pub unsafe fn poll_events(id: u64) -> Vec<scm::ScmEvent> {
    let mut events = Vec::new();
    let instances = SHIM_INSTANCES.lock();
    if let Some(instance) = instances.iter().find(|i| i.id == id) {
        let region_virt = crate::memory::vmm::phys_to_virt(instance.region_phys);
        let region = &*(region_virt as *const scm::ShimRegion);
        loop {
            let mut evt = scm::ScmEvent {
                evt_type: 0,
                id: 0,
                status: 0,
                actual_len: 0,
                data_ofs: 0,
                data_len: 0,
                reserved: 0,
            };
            if scm::scm_event_pop(&region.evt_queue, &mut evt) != 0 {
                break;
            }
            events.push(evt);
        }
    }
    events
}

pub fn init() {
    crate::printk!("[SHIM] Shim manager initialized");
}

pub unsafe fn load_driver_module(elf_data: &[u8], name: &str) -> Result<i32, &'static str> {
    let entry = loader::load_module(elf_data, name)?;
    let ret = loader::call_module_init(entry);
    Ok(ret)
}

pub unsafe fn unload_driver_module(name: &str) -> Result<(), &'static str> {
    loader::unload_module(name)
}
