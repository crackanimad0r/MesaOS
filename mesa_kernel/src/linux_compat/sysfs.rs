use alloc::format;
use alloc::string::String;

fn write_sys_file(path: &str, content: &str) {
    let _ = crate::fs::write(path, content.as_bytes());
}

fn ensure_sys_dir(path: &str) {
    let _ = crate::fs::mkdir(path);
}

pub fn populate() {
    let _ = crate::fs::mkdir("/sys");
    let _ = crate::fs::mkdir("/sys/kernel");
    let _ = crate::fs::mkdir("/sys/kernel/debug");
    let _ = crate::fs::mkdir("/sys/devices");
    let _ = crate::fs::mkdir("/sys/devices/system");
    let _ = crate::fs::mkdir("/sys/devices/system/cpu");
    let _ = crate::fs::mkdir("/sys/class");
    let _ = crate::fs::mkdir("/sys/class/net");
    let _ = crate::fs::mkdir("/sys/block");
    let _ = crate::fs::mkdir("/sys/power");
    let _ = crate::fs::mkdir("/sys/bus");
    let _ = crate::fs::mkdir("/sys/bus/pci");
    let _ = crate::fs::mkdir("/sys/bus/pci/devices");
    let _ = crate::fs::mkdir("/sys/fs");
    let _ = crate::fs::mkdir("/sys/fs/ramfs");

    populate_cpu();
    populate_net();
    populate_power();
    populate_block();
    populate_kernel_info();
    populate_fs_info();
}

fn populate_cpu() {
    let cpu_count = crate::limine_req::cpu_count();
    for i in 0..cpu_count {
        let dir = format!("/sys/devices/system/cpu/cpu{}", i);
        let _ = crate::fs::mkdir(&dir);
        write_sys_file(&format!("{}/online", dir), "1\n");
        write_sys_file(&format!("{}/present", dir), &format!("{}\n", i));
    }
    let cpu_present: String = if cpu_count > 1 {
        format!("0-{}\n", cpu_count - 1)
    } else {
        "0\n".into()
    };
    write_sys_file("/sys/devices/system/cpu/possible", &cpu_present);
    write_sys_file("/sys/devices/system/cpu/present", &cpu_present);
    write_sys_file("/sys/devices/system/cpu/online", &cpu_present);
    write_sys_file(
        "/sys/devices/system/cpu/kernel_max",
        &format!("{}\n", cpu_count - 1),
    );
}

fn populate_net() {
    write_sys_file("/sys/class/net/lo", "");
    let _ = crate::fs::mkdir("/sys/class/net/lo");
    write_sys_file("/sys/class/net/lo/address", "00:00:00:00:00:00\n");
    write_sys_file("/sys/class/net/lo/type", "772\n");
    write_sys_file("/sys/class/net/lo/operstate", "unknown\n");

    #[cfg(target_arch = "x86_64")]
    {
        let iface = if crate::net::is_virtio() {
            "enp0s3"
        } else {
            "eth0"
        };
        let _ = crate::fs::mkdir(&format!("/sys/class/net/{}", iface));
        write_sys_file(
            &format!("/sys/class/net/{}/address", iface),
            "52:54:00:12:34:56\n",
        );
        write_sys_file(&format!("/sys/class/net/{}/type", iface), "1\n");
        write_sys_file(&format!("/sys/class/net/{}/operstate", iface), "up\n");
        write_sys_file(&format!("/sys/class/net/{}/speed", iface), "1000\n");
        write_sys_file(&format!("/sys/class/net/{}/duplex", iface), "full\n");
    }
}

fn populate_power() {
    write_sys_file("/sys/power/state", "standby mem\n");
    write_sys_file("/sys/power/disk", "[platform]\n");
    write_sys_file("/sys/power/image_size", "0\n");
}

fn populate_block() {
    write_sys_file("/sys/block/ram0", "");
    let _ = crate::fs::mkdir("/sys/block/ram0");
    write_sys_file("/sys/block/ram0/size", "65536\n");
    write_sys_file("/sys/block/ram0/removable", "0\n");
    write_sys_file("/sys/block/ram0/ro", "0\n");
}

fn populate_kernel_info() {
    let hostname = crate::config::get_hostname();
    write_sys_file("/sys/kernel/hostname", &format!("{}\n", hostname));
    write_sys_file("/sys/kernel/ostype", "MesaOS\n");
    write_sys_file("/sys/kernel/osrelease", "0.1.0\n");
    write_sys_file("/sys/kernel/version", "#1 Tue Jun 9 00:00:00 UTC 2026\n");

    let (free, total) = crate::memory::pmm::stats();
    let total_mb = (total * crate::memory::PAGE_SIZE) / 1024 / 1024;
    write_sys_file("/sys/kernel/ram_total_mb", &format!("{}\n", total_mb));
    write_sys_file("/sys/kernel/debug/linux_compat", "Linux Compatibility Layer v0.1.0\nComandos: grep, find, head, tail, sort, wc, uname, env, which, basename, dirname, yes, sleep, seq\nSyscalls: read, write, open, close, stat, lseek, yield, sleep, getpid, exit, getuid, pipe\n");
}

fn populate_fs_info() {
    write_sys_file("/sys/fs/ramfs/max_pages", "65536\n");
    write_sys_file("/sys/fs/ramfs/max_inodes", "65536\n");
}
