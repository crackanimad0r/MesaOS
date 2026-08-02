use x86_64::instructions::port::Port;

pub unsafe fn inb(port: u16) -> u8 {
    let mut p = Port::new(port);
    p.read()
}

pub unsafe fn inw(port: u16) -> u16 {
    let mut p = Port::new(port);
    p.read()
}

pub unsafe fn inl(port: u16) -> u32 {
    let mut p: Port<u32> = Port::new(port);
    p.read()
}

pub unsafe fn outb(port: u16, val: u8) {
    let mut p = Port::new(port);
    p.write(val);
}

pub unsafe fn outw(port: u16, val: u16) {
    let mut p = Port::new(port);
    p.write(val);
}

pub unsafe fn outl(port: u16, val: u32) {
    let mut p: Port<u32> = Port::new(port);
    p.write(val);
}

pub unsafe fn ioread8(addr: *mut u8) -> u8 {
    addr.read_volatile()
}

pub unsafe fn iowrite8(addr: *mut u8, val: u8) {
    addr.write_volatile(val);
}

pub unsafe fn ioread32(addr: *mut u32) -> u32 {
    addr.read_volatile()
}

pub unsafe fn iowrite32(addr: *mut u32, val: u32) {
    addr.write_volatile(val);
}

pub unsafe fn ioread64(addr: *mut u64) -> u64 {
    addr.read_volatile()
}

pub unsafe fn iowrite64(addr: *mut u64, val: u64) {
    addr.write_volatile(val);
}
