use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

fn write_proc_file(path: &str, content: &str) {
    let _ = crate::fs::write(path, content.as_bytes());
}

fn ensure_proc_dir(path: &str) {
    let _ = crate::fs::mkdir(path);
}

pub fn populate() {
    let _ = crate::fs::mkdir("/proc");
    let _ = crate::fs::mkdir("/proc/self");

    populate_cpuinfo();
    populate_meminfo();
    populate_uptime();
    populate_version();
    populate_stat();
    populate_loadavg();
    populate_hostname();
    populate_mounts();
    populate_modules();
    populate_pci();
    populate_interrupts();
    populate_self();
}

fn populate_cpuinfo() {
    let count = crate::limine_req::cpu_count();
    let mut info = String::new();
    for i in 0..count {
        info.push_str(&format!(
            "processor\t: {}\nvendor_id\t: MesaOS\ncpu family\t: 1\nmodel\t\t: 1\nmodel name\t: MesaOS v0.1.0 Virtual CPU\nstepping\t: 1\ncpu MHz\t\t: 1.000\nfpu\t\t: yes\nfpu_exception\t: yes\ncpuid level\t: 1\nwp\t\t: yes\nflags\t\t: fpu de pse tsc msr pae mce cx8 apic sep mca cmov pat pse36 clflush mmx fxsr sse sse2 syscall nx rdtscp\nbogomips\t: 1.00\nclflush size\t: 64\ncache_alignment\t: 64\naddress sizes\t: 39 bits physical, 48 bits virtual\npower management:\n\n",
            i
        ));
    }
    write_proc_file("/proc/cpuinfo", &info);
}

fn populate_meminfo() {
    let (free, total) = crate::memory::pmm::stats();
    let page_size = crate::memory::PAGE_SIZE as u64;
    let total_bytes = total * page_size;
    let free_bytes = free * page_size;
    let used_bytes = total_bytes - free_bytes;

    let total_kb = total_bytes / 1024;
    let free_kb = free_bytes / 1024;
    let used_kb = used_bytes / 1024;
    let available_kb = free_kb;

    let info = format!(
        "MemTotal:       {:>8} kB\nMemFree:        {:>8} kB\nMemAvailable:   {:>8} kB\nBuffers:        {:>8} kB\nCached:         {:>8} kB\nSwapCached:     {:>8} kB\nActive:         {:>8} kB\nInactive:       {:>8} kB\nSwapTotal:      {:>8} kB\nSwapFree:       {:>8} kB\nDirty:          {:>8} kB\nWriteback:      {:>8} kB\nAnonPages:      {:>8} kB\nMapped:         {:>8} kB\nSlab:           {:>8} kB\nPageTables:     {:>8} kB\nVmallocTotal:   {:>8} kB\nVmallocUsed:    {:>8} kB\nHugePages_Total:       0\nHugePages_Free:        0\n",
        total_kb, free_kb, available_kb, 0u64, 0u64, 0u64, used_kb, 0u64,
        0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 33554432u64, 0u64
    );
    write_proc_file("/proc/meminfo", &info);
}

fn populate_uptime() {
    let ticks = crate::curr_arch::get_ticks();
    let seconds = ticks / 18;
    let idle = 0u64;
    let info = format!(
        "{}.{:02} {}.{:02}\n",
        seconds / 100,
        seconds % 100,
        idle / 100,
        idle % 100
    );
    write_proc_file("/proc/uptime", &info);
}

fn populate_version() {
    let info = "Linux version 0.1.0 (mesa@mesa-os) (gcc (GCC) 12.2.0) #1 SMP Tue Jun 9 00:00:00 UTC 2026\n";
    write_proc_file("/proc/version", info);
}

fn populate_stat() {
    let ticks = crate::curr_arch::get_ticks();
    let uptime_ticks = ticks;
    let user = uptime_ticks / 10;
    let nice = 0u64;
    let system = uptime_ticks / 20;
    let idle = uptime_ticks;
    let iowait = 0u64;
    let irq = 0u64;
    let softirq = 0u64;
    let steal = 0u64;

    let cpu_count = crate::limine_req::cpu_count();
    let procs_running = 1u64;
    let procs_blocked = 0u64;
    let processes = crate::scheduler::task_count();

    let info = format!(
        "cpu  {} {} {} {} {} {} {} {} 0 0\n\
         cpu0 {} {} {} {} {} {} {} {} 0 0\n\
         intr 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n\
         ctxt 0\n\
         btime 0\n\
         processes {}\n\
         procs_running {}\n\
         procs_blocked {}\n\
         softirq 0 0 0 0 0 0 0 0 0 0 0\n",
        user,
        nice,
        system,
        idle,
        iowait,
        irq,
        softirq,
        steal,
        user,
        nice,
        system,
        idle,
        iowait,
        irq,
        softirq,
        steal,
        processes,
        procs_running,
        procs_blocked
    );
    write_proc_file("/proc/stat", &info);
}

fn populate_loadavg() {
    let info = "0.00 0.00 0.00 1/1 1\n";
    write_proc_file("/proc/loadavg", info);
}

fn populate_hostname() {
    let hostname = crate::config::get_hostname();
    write_proc_file("/proc/sys", "");
    let _ = crate::fs::mkdir("/proc/sys");
    let _ = crate::fs::mkdir("/proc/sys/kernel");
    write_proc_file("/proc/sys/kernel/hostname", &format!("{}\n", hostname));
    write_proc_file("/proc/sys/kernel/ostype", "MesaOS\n");
    write_proc_file("/proc/sys/kernel/osrelease", "0.1.0\n");
    write_proc_file(
        "/proc/sys/kernel/version",
        "#1 Tue Jun 9 00:00:00 UTC 2026\n",
    );
}

fn populate_mounts() {
    let info = "rootfs / rootfs rw 0 0\nproc /proc proc rw 0 0\nsysfs /sys sysfs rw 0 0\n";
    write_proc_file("/proc/mounts", info);
}

fn populate_modules() {
    let mut info = String::new();
    let modules = crate::shim::loader::list_loaded_modules();
    for m in &modules {
        let size_kb = (m.size + 1023) / 1024;
        let deps = if m.depends.is_empty() {
            "-".to_string()
        } else {
            m.depends.join(",")
        };
        info.push_str(&format!(
            "{} {} {} {} {}\n",
            m.name, size_kb, m.refcount, 0, deps
        ));
    }
    if info.is_empty() {
        info = "(no modules loaded)\n".to_string();
    }
    write_proc_file("/proc/modules", &info);
}

fn populate_pci() {
    let mut info = String::new();
    #[cfg(target_arch = "x86_64")]
    {
        let devices = crate::pci::devices();
        for dev in &devices {
            info.push_str(&format!(
                "{:02x}:{:02x}.{} Class {:04x}: Vendor={:04x} Device={:04x}\n",
                dev.bus,
                dev.device,
                dev.function,
                dev.class_code as u32,
                dev.vendor_id,
                dev.device_id
            ));
        }
    }
    write_proc_file("/proc/pci", &info);
}

fn populate_interrupts() {
    let cpu_count = crate::limine_req::cpu_count();
    let mut info = String::new();
    info.push_str("           CPU0");
    for i in 1..cpu_count {
        info.push_str(&format!("       CPU{}", i));
    }
    info.push_str("   \n");

    info.push_str(&format!(
        "  0:        0          {}   IO-APIC   2-edge      timer\n\
          1:        0          {}   IO-APIC   1-edge      i8042\n\
          8:        0          {}   IO-APIC   8-edge      rtc0\n\
          9:        0          {}   IO-APIC   9-fasteoi   acpi\n\
         12:        0          {}   IO-APIC  12-edge      i8042\n\
         14:        0          {}   IO-APIC  14-edge      ata_piix\n\
         15:        0          {}   IO-APIC  15-edge      ata_piix\n",
        cpu_count, cpu_count, cpu_count, cpu_count, cpu_count, cpu_count, cpu_count
    ));
    write_proc_file("/proc/interrupts", &info);
}

fn populate_self() {
    let _ = crate::fs::write("/proc/self/exe", b"/bin/mesa-sh\n");
}
