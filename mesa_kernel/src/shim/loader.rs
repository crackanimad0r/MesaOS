use super::symbols;
use crate::linux::*;
use crate::memory::{
    address_space::{self, flags, AddressSpace},
    pmm, vmm, PAGE_SIZE,
};
use crate::printk;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

const ET_REL: u16 = 1;
const EM_X86_64: u16 = 62;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHT_PROGBITS: u32 = 1;
const SHT_NOBITS: u32 = 8;
const SHF_ALLOC: u64 = 0x2;
const SHT_INIT_ARRAY: u32 = 14;
const SHT_FINI_ARRAY: u32 = 15;
const SHT_DYNAMIC: u32 = 6;
const SHT_NULL: u32 = 0;

// Relocation types (x86_64)
const R_X86_64_64: u32 = 1;
const R_X86_64_PC32: u32 = 2;
const R_X86_64_GOT32: u32 = 3;
const R_X86_64_PLT32: u32 = 4;
const R_X86_64_COPY: u32 = 5;
const R_X86_64_GLOB_DAT: u32 = 6;
const R_X86_64_JUMP_SLOT: u32 = 7;
const R_X86_64_RELATIVE: u32 = 8;
const R_X86_64_GOTPCREL: u32 = 9;
const R_X86_64_32: u32 = 10;
const R_X86_64_32S: u32 = 11;
const R_X86_64_PC64: u32 = 24;
const R_X86_64_GOT64: u32 = 25;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

pub unsafe fn call_module_init(entry: u64) -> i32 {
    if entry == 0 {
        printk!("[SHIM] No init function for module");
        return 0;
    }
    let func: extern "C" fn() -> i32 = core::mem::transmute(entry);
    let start = crate::curr_arch::get_ticks();
    printk!(
        "[SHIM] Calling module init at {:#x} (ticks={})",
        entry,
        start
    );
    printk!("[SHIM] Module init running... (may take a while, Ctrl+C not yet supported)");
    let ret = func();
    let elapsed = crate::curr_arch::get_ticks().wrapping_sub(start);
    printk!(
        "[SHIM] Module init returned {} (took {} ticks)",
        ret,
        elapsed
    );
    ret
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Elf64Shdr {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u64,
    sh_addr: u64,
    sh_offset: u64,
    sh_size: u64,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u64,
    sh_entsize: u64,
}

#[repr(C, packed)]
struct Elf64Sym {
    st_name: u32,
    st_info: u8,
    st_other: u8,
    st_shndx: u16,
    st_value: u64,
    st_size: u64,
}

#[repr(C, packed)]
struct Elf64Rela {
    r_offset: u64,
    r_info: u64,
    r_addend: i64,
}

#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub name: String,
    pub base_addr: u64,
    pub phys_base: u64,
    pub size: u64,
    pub entry_point: u64,
    pub exit_point: u64,
    pub sections: Vec<ModuleSection>,
    pub init_array: Vec<u64>,
    pub fini_array: Vec<u64>,
    pub depends: Vec<String>,
    pub refcount: u32,
}

#[derive(Debug, Clone)]
pub struct ModuleSection {
    pub name: String,
    pub vaddr: u64,
    pub size: u64,
    pub flags: u64,
}

static LOADED_MODULES: Mutex<Vec<LoadedModule>> = Mutex::new(Vec::new());
static NEXT_MODULE_ADDR: AtomicU64 = AtomicU64::new(0xFFFFFFFF81000000);

struct SectionInfo {
    shdr: Elf64Shdr,
    data: Vec<u8>,
    allocated_addr: u64,
    name: String,
}

fn get_section_name(elf: &[u8], shdr: &Elf64Shdr, shstrtab: &[u8]) -> String {
    let mut name = String::new();
    let mut pos = shdr.sh_name as usize;
    while pos < shstrtab.len() {
        let c = shstrtab[pos];
        if c == 0 {
            break;
        }
        name.push(c as char);
        pos += 1;
    }
    name
}

fn find_section(elf: &[u8], name: &str) -> Option<(usize, Elf64Shdr)> {
    if elf.len() < 64 {
        return None;
    }
    let hdr = unsafe { &*(elf.as_ptr() as *const Elf64Ehdr) };
    let shoff = hdr.e_shoff as usize;
    let shnum = hdr.e_shnum as usize;
    let shentsize = hdr.e_shentsize as usize;
    if shoff == 0 || shnum == 0 {
        return None;
    }

    let strtab_idx = hdr.e_shstrndx as usize;
    let strtab_off = shoff + strtab_idx * shentsize;
    if strtab_off + shentsize > elf.len() {
        return None;
    }
    let strtab_shdr = unsafe { &*(elf.as_ptr().add(strtab_off) as *const Elf64Shdr) };
    let strtab_start = strtab_shdr.sh_offset as usize;
    let strtab_end = strtab_start + strtab_shdr.sh_size as usize;
    if strtab_end > elf.len() {
        return None;
    }
    let shstrtab = &elf[strtab_start..strtab_end];

    for i in 0..shnum {
        let off = shoff + i * shentsize;
        if off + shentsize > elf.len() {
            break;
        }
        let section = unsafe { &*(elf.as_ptr().add(off) as *const Elf64Shdr) };
        let sname = get_section_name(elf, section, shstrtab);
        if sname == name {
            return Some((i, *section));
        }
    }
    None
}

pub unsafe fn load_module(elf_data: &[u8], name: &str) -> Result<u64, &'static str> {
    if elf_data.len() < 64 {
        return Err("Module too small");
    }

    let hdr = unsafe { &*(elf_data.as_ptr() as *const Elf64Ehdr) };
    if hdr.e_ident[0..4] != [0x7f, b'E', b'L', b'F'] {
        return Err("Bad ELF magic");
    }
    if hdr.e_type != ET_REL {
        return Err("Not a relocatable object (.ko)");
    }
    if hdr.e_machine != EM_X86_64 {
        return Err("Not x86_64");
    }

    let shoff = hdr.e_shoff as usize;
    let shnum = hdr.e_shnum as usize;
    let shentsize = hdr.e_shentsize as usize;

    if shoff == 0 || shnum == 0 {
        return Err("No section headers");
    }

    let strtab_idx = hdr.e_shstrndx as usize;
    let strtab_off = shoff + strtab_idx * shentsize;
    if strtab_off + shentsize > elf_data.len() {
        return Err("String table header out of bounds");
    }
    let strtab_shdr = unsafe { &*(elf_data.as_ptr().add(strtab_off) as *const Elf64Shdr) };
    let shstrtab = &elf_data
        [strtab_shdr.sh_offset as usize..(strtab_shdr.sh_offset + strtab_shdr.sh_size) as usize];

    let mut sections: Vec<SectionInfo> = Vec::new();

    for i in 0..shnum {
        let off = shoff + i * shentsize;
        if off + shentsize > elf_data.len() {
            return Err("Section header out of bounds");
        }
        let shdr = unsafe { &*(elf_data.as_ptr().add(off) as *const Elf64Shdr) };
        let sname = get_section_name(elf_data, shdr, shstrtab);

        let sdata = if shdr.sh_type == SHT_NOBITS {
            Vec::new()
        } else if shdr.sh_type != SHT_SYMTAB
            && shdr.sh_type != SHT_STRTAB
            && shdr.sh_type != SHT_RELA
        {
            let start = shdr.sh_offset as usize;
            let end = start + shdr.sh_size as usize;
            if end > elf_data.len() {
                Vec::new()
            } else {
                elf_data[start..end].to_vec()
            }
        } else {
            Vec::new()
        };

        sections.push(SectionInfo {
            shdr: *shdr,
            data: sdata,
            allocated_addr: 0,
            name: sname,
        });
    }

    // Extract module name from .gnu.linkonce.this_module section if present
    let module_name = {
        let mut mn = name.to_string();
        for sec in &sections {
            if sec.name == ".gnu.linkonce.this_module" {
                // struct module.name is at offset 24 (Linux 6.x)
                let name_start = 24usize;
                let name_end = name_start + 64;
                if name_end <= sec.data.len() {
                    let name_bytes = &sec.data[name_start..name_end];
                    let actual_len = name_bytes.iter().position(|&c| c == 0).unwrap_or(64);
                    if actual_len > 0 {
                        if let Ok(n) = core::str::from_utf8(&name_bytes[..actual_len]) {
                            mn = n.to_string();
                        }
                    }
                }
                break;
            }
        }
        mn
    };

    // Parse .modinfo section for dependency info
    let module_depends = {
        let mut deps: Vec<String> = Vec::new();
        for sec in &sections {
            if sec.name == ".modinfo" && !sec.data.is_empty() {
                let bytes = &sec.data;
                let mut i = 0;
                while i < bytes.len() {
                    let end = bytes[i..]
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(bytes.len() - i);
                    if end > 0 {
                        if let Ok(entry) = core::str::from_utf8(&bytes[i..i + end]) {
                            if let Some(dep_str) = entry.strip_prefix("depends=") {
                                for d in dep_str.split(',') {
                                    let d = d.trim();
                                    if !d.is_empty() {
                                        deps.push(d.to_string());
                                    }
                                }
                            }
                        }
                    }
                    i += end + 1;
                }
                break;
            }
        }
        deps
    };

    // Check module dependencies
    if !module_depends.is_empty() {
        let loaded = LOADED_MODULES.lock();
        for dep in &module_depends {
            if !loaded.iter().any(|m| m.name == *dep) {
                return Err("Missing module dependency");
            }
        }
    }

    let mut total_alloc = 0u64;
    for sec in &sections {
        if (sec.shdr.sh_flags & SHF_ALLOC) != 0 {
            let size = if sec.shdr.sh_type == SHT_NOBITS {
                sec.shdr.sh_size
            } else {
                sec.shdr.sh_size
            };
            total_alloc += (size + 4095) & !4095;
        }
    }

    if total_alloc == 0 {
        return Err("No allocable sections");
    }

    let total_pages = ((total_alloc + 4095) / 4096) as usize;
    let phys_frames = pmm::alloc_frames(total_pages).ok_or("No memory for module")?;
    let module_virt = NEXT_MODULE_ADDR.fetch_add(total_pages as u64 * PAGE_SIZE, Ordering::Relaxed);

    let mut kernel_as = AddressSpace::kernel();
    for i in 0..total_pages {
        let phys = phys_frames + (i as u64) * PAGE_SIZE;
        let virt = module_virt + (i as u64) * PAGE_SIZE;
        kernel_as.map_page(virt, phys, flags::KERNEL_RW | flags::KERNEL_RX)?;
    }

    let mut current_offset = 0u64;
    let mut allocated_sections: Vec<ModuleSection> = Vec::new();

    for sec in &mut sections {
        if (sec.shdr.sh_flags & SHF_ALLOC) != 0 {
            let aligned = (current_offset + 4095) & !4095;
            current_offset = aligned;
            sec.allocated_addr = module_virt + current_offset;
            let size_pages = ((sec.shdr.sh_size + 4095) / 4096) * 4096;

            if sec.shdr.sh_type != SHT_NOBITS && !sec.data.is_empty() {
                let hhdm_base = vmm::phys_to_virt(phys_frames) + aligned;
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        sec.data.as_ptr(),
                        hhdm_base as *mut u8,
                        sec.data.len().min(sec.shdr.sh_size as usize),
                    );
                }
            }

            allocated_sections.push(ModuleSection {
                name: sec.name.clone(),
                vaddr: sec.allocated_addr,
                size: sec.shdr.sh_size,
                flags: sec.shdr.sh_flags,
            });

            current_offset += size_pages;
        }
    }

    let mut unresolved_symbol = false;

    for i in 0..shnum {
        let off = shoff + i * shentsize;
        if off + shentsize > elf_data.len() {
            continue;
        }
        let shdr = unsafe { &*(elf_data.as_ptr().add(off) as *const Elf64Shdr) };

        if shdr.sh_type != SHT_RELA {
            continue;
        }

        let symtab_idx = shdr.sh_link as usize;
        if symtab_idx >= sections.len() {
            continue;
        }

        let symtab_start = sections[symtab_idx].shdr.sh_offset as usize;
        let symtab_size = sections[symtab_idx].shdr.sh_size as usize;
        let sym_entsize = sections[symtab_idx].shdr.sh_entsize as usize;
        if sym_entsize == 0 {
            continue;
        }

        let strtab_link = sections[symtab_idx].shdr.sh_link as usize;
        let linked_strtab_start: usize;
        if strtab_link < sections.len() {
            linked_strtab_start = sections[strtab_link].shdr.sh_offset as usize;
        } else {
            continue;
        }

        let rel_start = shdr.sh_offset as usize;
        let rel_size = shdr.sh_size as usize;
        let rel_entsize = shdr.sh_entsize as usize;
        if rel_entsize == 0 {
            continue;
        }

        let target_section = shdr.sh_info as usize;
        if target_section >= sections.len() {
            continue;
        }

        let target_base = sections[target_section].allocated_addr;
        if target_base == 0 {
            continue;
        }

        let rel_data = &elf_data[rel_start..rel_start + rel_size];
        let num_rel = rel_size / rel_entsize;

        for r in 0..num_rel {
            let rela_offset = r * rel_entsize;
            if rela_offset + core::mem::size_of::<Elf64Rela>() > rel_data.len() {
                continue;
            }
            let rela = unsafe { &*(rel_data.as_ptr().add(rela_offset) as *const Elf64Rela) };

            let r_type = (rela.r_info & 0xFFFFFFFF) as u32;
            let r_sym = (rela.r_info >> 32) as usize;

            let sym_offset = r_sym * sym_entsize;
            if sym_offset + core::mem::size_of::<Elf64Sym>() > symtab_start + symtab_size {
                continue;
            }

            let sym_data = &elf_data[symtab_start..symtab_start + symtab_size];
            let sym = unsafe { &*(sym_data.as_ptr().add(sym_offset) as *const Elf64Sym) };

            let sym_name = if sym.st_name != 0
                && (linked_strtab_start + sym.st_name as usize) < elf_data.len()
            {
                let mut sn = String::new();
                let mut pos = linked_strtab_start + sym.st_name as usize;
                while pos < elf_data.len() {
                    let c = elf_data[pos];
                    if c == 0 {
                        break;
                    }
                    sn.push(c as char);
                    pos += 1;
                }
                sn
            } else {
                String::new()
            };

            let patch_addr = target_base + rela.r_offset;

            let sym_value = if sym.st_shndx < shnum as u16 && sym.st_shndx != 0 {
                let sec_idx = sym.st_shndx as usize;
                sections[sec_idx].allocated_addr + sym.st_value
            } else if !sym_name.is_empty() {
                match symbols::find_symbol(&sym_name) {
                    Some(addr) => addr as u64,
                    None => {
                        printk!("[SHIM] ERROR: unresolved symbol: {}", sym_name);
                        unresolved_symbol = true;
                        0
                    }
                }
            } else {
                sym.st_value
            };

            match r_type {
                R_X86_64_64 | R_X86_64_GLOB_DAT | R_X86_64_JUMP_SLOT => unsafe {
                    let ptr = patch_addr as *mut u64;
                    *ptr = sym_value.wrapping_add(rela.r_addend as u64);
                },
                R_X86_64_RELATIVE => unsafe {
                    let ptr = patch_addr as *mut u64;
                    *ptr = target_base.wrapping_add(rela.r_addend as u64);
                },
                R_X86_64_PC32 | R_X86_64_PLT32 | R_X86_64_GOTPCREL => {
                    let value = (sym_value.wrapping_add(rela.r_addend as u64))
                        .wrapping_sub(patch_addr) as u32;
                    unsafe {
                        let ptr = patch_addr as *mut u32;
                        *ptr = value;
                    }
                }
                R_X86_64_PC64 => {
                    let value =
                        (sym_value.wrapping_add(rela.r_addend as u64)).wrapping_sub(patch_addr);
                    unsafe {
                        let ptr = patch_addr as *mut u64;
                        *ptr = value;
                    }
                }
                R_X86_64_32 | R_X86_64_32S => {
                    let value = sym_value.wrapping_add(rela.r_addend as u64) as u32;
                    unsafe {
                        let ptr = patch_addr as *mut u32;
                        *ptr = value;
                    }
                }
                R_X86_64_GOT64 => unsafe {
                    let ptr = patch_addr as *mut u64;
                    *ptr = sym_value.wrapping_add(rela.r_addend as u64);
                },
                _ => {}
            }
        }
    }

    if unresolved_symbol {
        return Err("Unresolved symbols in module");
    }

    // Collect init_array and fini_array entries
    let mut init_array: Vec<u64> = Vec::new();
    let mut fini_array: Vec<u64> = Vec::new();
    for sec in &sections {
        if sec.allocated_addr == 0 || sec.data.is_empty() {
            continue;
        }
        if sec.shdr.sh_type == SHT_INIT_ARRAY {
            let num_entries = sec.data.len() / 8;
            for i in 0..num_entries {
                let ptr = sec.allocated_addr + (i as u64) * 8;
                let func = unsafe { *(ptr as *const u64) };
                if func != 0 {
                    init_array.push(func);
                }
            }
            printk!("[SHIM] Found {} init_array entries", init_array.len());
        }
        if sec.shdr.sh_type == SHT_FINI_ARRAY {
            let num_entries = sec.data.len() / 8;
            for i in 0..num_entries {
                let ptr = sec.allocated_addr + (i as u64) * 8;
                let func = unsafe { *(ptr as *const u64) };
                if func != 0 {
                    fini_array.push(func);
                }
            }
            printk!("[SHIM] Found {} fini_array entries", fini_array.len());
        }
    }

    let entry_point = {
        let mut ep = 0u64;
        let mut exit_ep = 0u64;
        for i in 0..shnum {
            let off = shoff + i * shentsize;
            if off + shentsize > elf_data.len() {
                break;
            }
            let shdr = unsafe { &*(elf_data.as_ptr().add(off) as *const Elf64Shdr) };
            if shdr.sh_type != SHT_SYMTAB {
                continue;
            }

            let sym_start = shdr.sh_offset as usize;
            let sym_size = shdr.sh_size as usize;
            let sym_entsize = shdr.sh_entsize as usize;
            if sym_entsize < core::mem::size_of::<Elf64Sym>() {
                continue;
            }

            let strtab_link = shdr.sh_link as usize;
            let strtab_off = shoff + strtab_link * shentsize;
            if strtab_off + shentsize > elf_data.len() {
                continue;
            }
            let strtab_shdr = unsafe { &*(elf_data.as_ptr().add(strtab_off) as *const Elf64Shdr) };
            let strtab = &elf_data[strtab_shdr.sh_offset as usize
                ..(strtab_shdr.sh_offset as usize + strtab_shdr.sh_size as usize)
                    .min(elf_data.len())];

            let num_syms = sym_size / sym_entsize;
            for s in 0..num_syms {
                let sym_off = sym_start + s * sym_entsize;
                if sym_off + core::mem::size_of::<Elf64Sym>() > elf_data.len() {
                    continue;
                }
                let sym = unsafe { &*(elf_data.as_ptr().add(sym_off) as *const Elf64Sym) };
                if sym.st_name == 0 {
                    continue;
                }
                let mut sn = alloc::string::String::new();
                let mut pos = sym.st_name as usize;
                while pos < strtab.len() {
                    let c = strtab[pos];
                    if c == 0 {
                        break;
                    }
                    sn.push(c as char);
                    pos += 1;
                }
                if sn == "init_module" {
                    let sec_idx = sym.st_shndx as usize;
                    if sec_idx < sections.len() && sections[sec_idx].allocated_addr != 0 {
                        ep = sections[sec_idx].allocated_addr + sym.st_value;
                        let shndx = sym.st_shndx;
                        let st_val = sym.st_value;
                        printk!(
                            "[SHIM] Found {} symbol at {:#x} (section {}, +{:#x})",
                            sn,
                            ep,
                            shndx,
                            st_val
                        );
                    }
                } else if sn == "cleanup_module" {
                    let sec_idx = sym.st_shndx as usize;
                    if sec_idx < sections.len() && sections[sec_idx].allocated_addr != 0 {
                        exit_ep = sections[sec_idx].allocated_addr + sym.st_value;
                        let shndx = sym.st_shndx;
                        let st_val = sym.st_value;
                        printk!(
                            "[SHIM] Found cleanup_module symbol at {:#x} (section {}, +{:#x})",
                            exit_ep,
                            shndx,
                            st_val
                        );
                    }
                }
            }
        }
        if ep == 0 {
            // Fallback: section start
            if let Some((_, sec)) = find_section(elf_data, ".init.text") {
                if let Some(target) = sections
                    .iter()
                    .find(|s| s.shdr.sh_offset == sec.sh_offset && s.shdr.sh_size == sec.sh_size)
                {
                    ep = target.allocated_addr;
                }
            }
        }
        (ep, exit_ep)
    };

    let (entry_point, cleanup_point) = entry_point;

    let module = LoadedModule {
        name: module_name,
        base_addr: module_virt,
        phys_base: phys_frames,
        size: total_alloc,
        entry_point,
        exit_point: cleanup_point,
        sections: allocated_sections,
        init_array,
        fini_array,
        depends: module_depends,
        refcount: 0,
    };

    LOADED_MODULES.lock().push(module.clone());

    printk!(
        "[SHIM] Module '{}' loaded at {:#x}, size={:#x}, entry={:#x}",
        name,
        module_virt,
        total_alloc,
        entry_point
    );

    Ok(entry_point)
}

pub unsafe fn unload_module(name: &str) -> Result<(), &'static str> {
    let mut modules = LOADED_MODULES.lock();
    // Check if any loaded module depends on this one
    for m in modules.iter() {
        if m.depends.iter().any(|d| d == name) {
            printk!("[SHIM] Cannot unload '{}': depends by '{}'", name, m.name);
            return Err("Module in use by another module");
        }
    }
    if let Some(pos) = modules.iter().position(|m| m.name == name) {
        let module = modules.remove(pos);
        if module.refcount > 0 {
            printk!(
                "[SHIM] Cannot unload '{}': refcount={}",
                name,
                module.refcount
            );
            return Err("Module busy");
        }
        // Call exit function if present
        if module.exit_point != 0 {
            let exit_func: extern "C" fn() = core::mem::transmute(module.exit_point);
            printk!("[SHIM] Calling module exit at {:#x}", module.exit_point);
            exit_func();
        }
        // Call fini_array entries (destructors)
        for &func_ptr in &module.fini_array {
            let func: extern "C" fn() = core::mem::transmute(func_ptr);
            printk!("[SHIM] Calling fini_array entry at {:#x}", func_ptr);
            func();
        }
        let frames = ((module.size + 4095) / 4096) as usize;
        let page_size = crate::memory::PAGE_SIZE as u64;
        for i in 0..frames {
            crate::memory::pmm::free_frame(module.phys_base + (i as u64) * page_size);
        }
        printk!("[SHIM] Module '{}' unloaded", name);
        Ok(())
    } else {
        Err("Module not found")
    }
}

pub fn try_module_get(name: &str) -> bool {
    let mut modules = LOADED_MODULES.lock();
    if let Some(m) = modules.iter_mut().find(|m| m.name == name) {
        m.refcount = m.refcount.wrapping_add(1);
        true
    } else {
        false
    }
}

pub fn module_put(name: &str) {
    let mut modules = LOADED_MODULES.lock();
    if let Some(m) = modules.iter_mut().find(|m| m.name == name) {
        if m.refcount > 0 {
            m.refcount -= 1;
        }
    }
}

pub fn module_refcount(name: &str) -> u32 {
    let modules = LOADED_MODULES.lock();
    modules
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.refcount)
        .unwrap_or(0)
}

pub fn any_module_depends_on(name: &str) -> bool {
    let modules = LOADED_MODULES.lock();
    modules.iter().any(|m| m.depends.iter().any(|d| d == name))
}

pub unsafe fn call_module_init_full(module: &LoadedModule) -> i32 {
    // First call all init_array entries (constructors)
    for &func_ptr in &module.init_array {
        let func: extern "C" fn() = core::mem::transmute(func_ptr);
        printk!("[SHIM] Calling init_array entry at {:#x}", func_ptr);
        func();
    }
    // Then call the main init function
    call_module_init(module.entry_point)
}

pub fn list_loaded_modules() -> Vec<LoadedModule> {
    LOADED_MODULES.lock().clone()
}

pub fn find_module(name: &str) -> Option<LoadedModule> {
    LOADED_MODULES
        .lock()
        .iter()
        .find(|m| m.name == name)
        .cloned()
}
