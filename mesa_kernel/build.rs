fn main() {
    // Compile UEFI NVRAM C implementation
    cc::Build::new()
        .file("src/arch/x86_64/uefi_nvram.c")
        .include("src/arch/x86_64")
        .flag("-ffreestanding")
        .flag("-nostdlib")
        .flag("-mno-red-zone")
        .flag("-fno-PIC")
        .flag("-mno-sse")
        .compile("uefi_nvram");

    // Compile USB Shim C implementation (Linux driver compatibility layer)
    cc::Build::new()
        .file("src/shim/c_api/usb_shim_pool.c")
        .file("src/shim/c_api/usb_shim_xhci.c")
        .file("src/shim/c_api/usb_shim_ports.c")
        .file("src/shim/c_api/usb_shim_urb.c")
        .file("src/shim/c_api/usb_shim_commands.c")
        .file("src/shim/c_api/usb_shim_api.c")
        .file("src/shim/c_api/usb_shim_descriptors.c")
        .file("src/shim/c_api/usb_shim_main.c")
        .include("src/shim/c_api")
        .flag("-ffreestanding")
        .flag("-nostdlib")
        .flag("-mno-red-zone")
        .flag("-fno-PIC")
        .flag("-mno-sse")
        .flag("-std=c11")
        .compile("usb_shim");

    println!("cargo:rerun-if-changed=src/arch/x86_64/uefi_nvram.c");
    println!("cargo:rerun-if-changed=src/arch/x86_64/uefi_nvram.h");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim.h");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_types.h");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_core.h");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_xhci.h");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_pool.c");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_xhci.c");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_ports.c");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_urb.c");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_commands.c");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_api.c");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_descriptors.c");
    println!("cargo:rerun-if-changed=src/shim/c_api/usb_shim_main.c");
}
