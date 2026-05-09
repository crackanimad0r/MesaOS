//! Sistema para ejecutar código de usuario como proceso separado

/// Ejecuta código de usuario en una nueva tarea con espacio de direcciones propio
pub fn exec_user_code(name: &str, code: &'static [u8]) -> u64 {
    exec_user_from_slice(name, code)
}

/// Ejecuta desde un slice (bytecode o ELF64). Usado para cargar binarios desde archivo.
pub fn exec_user_from_slice(name: &str, code: &[u8]) -> u64 {
    crate::serial_println!("[EXEC] Creating user process '{}' ({} bytes)", name, code.len());
    
    match crate::scheduler::spawn_user(name, code) {
        Ok(id) => {
            crate::serial_println!("[EXEC] User process '{}' created with PID {}", name, id);
            id
        }
        Err(e) => {
            crate::serial_println!("[EXEC] Failed to create user process: {}", e);
            0
        }
    }
}


/// Programas de usuario disponibles
pub mod programs {
    /// Hello World en Ring 3
    #[rustfmt::skip]
    pub const HELLO: [u8; 64] = [
        // mov rax, 1 (sys_write)
        0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00,
        // mov rdi, 1 (stdout)
        0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00,
        // lea rsi, [rip+25]
        0x48, 0x8d, 0x35, 0x19, 0x00, 0x00, 0x00,
        // mov rdx, 20
        0x48, 0xc7, 0xc2, 0x14, 0x00, 0x00, 0x00,
        // syscall
        0x0f, 0x05,
        // mov rax, 60 (sys_exit)
        0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,
        // xor rdi, rdi
        0x48, 0x31, 0xff,
        // syscall
        0x0f, 0x05,
        // jmp $
        0xeb, 0xfe,
        // String: "Hello from Ring 3!\n"
        b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r',
        b'o', b'm', b' ', b'R', b'i', b'n', b'g', b' ',
        b'3', b'!', b'\n', 0x00,
    ];
    
    /// Counter: Imprime 3 veces y sale
    #[rustfmt::skip]
    pub const COUNTER: [u8; 76] = [
        // mov r12, 0 (counter)
        0x49, 0xc7, 0xc4, 0x00, 0x00, 0x00, 0x00,
        // loop:
        // mov rax, 1 (sys_write)
        0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00,
        // mov rdi, 1
        0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00,
        // lea rsi, [rip+40]
        0x48, 0x8d, 0x35, 0x28, 0x00, 0x00, 0x00,
        // mov rdx, 10
        0x48, 0xc7, 0xc2, 0x0a, 0x00, 0x00, 0x00,
        // syscall
        0x0f, 0x05,
        // inc r12
        0x49, 0xff, 0xc4,
        // cmp r12, 3
        0x49, 0x83, 0xfc, 0x03,
        // jl loop (-38 bytes)
        0x7c, 0xd8,
        // mov rax, 60 (sys_exit)
        0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,
        // xor rdi, rdi
        0x48, 0x31, 0xff,
        // syscall
        0x0f, 0x05,
        // jmp $
        0xeb, 0xfe,
        // String: "Counter!\n\0"
        b'C', b'o', b'u', b'n', b't', b'e', b'r', b'!', b'\n', 0x00,
        // Padding (6 bytes para completar 76)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    /// Hello World ELF (Embedded)
    pub const HELLO_ELF: &[u8] = include_bytes!("../../../userland/hello_elf/hello.elf");

    /// File Test: Abre /etc/hostname, lo lee y lo imprime (Ring 3)
    #[rustfmt::skip]
    pub const FILE_TEST: [u8; 157] = [
        // 0: mov rax, 2 (sys_open)
        0x48, 0xc7, 0xc0, 0x02, 0x00, 0x00, 0x00,
        // 7: lea rdi, [rip+90] (Path) -> RIP=14. 14+90=104.
        0x48, 0x8d, 0x3d, 0x5a, 0x00, 0x00, 0x00,
        // 14: xor rsi, rsi (O_RDONLY)
        0x48, 0x31, 0xf6,
        // 17: syscall
        0x0f, 0x05,
        
        // 19: mov r12, rax (save FD)
        0x49, 0x89, 0xc4,
        
        // 22: mov rax, 0 (sys_read)
        0x48, 0xc7, 0xc0, 0x00, 0x00, 0x00, 0x00,
        // 29: mov rdi, r12 (FD)
        0x4c, 0x89, 0xe7,
        // 32: lea rsi, [rip+86] (Buffer) -> RIP=39. 39+86=125.
        0x48, 0x8d, 0x35, 0x56, 0x00, 0x00, 0x00,
        // 39: mov rdx, 32 (count)
        0x48, 0xc7, 0xc2, 0x20, 0x00, 0x00, 0x00,
        // 46: syscall
        0x0f, 0x05,
        
        // 48: mov r13, rax (bytes read)
        0x49, 0x89, 0xc5,
        
        // 51: mov rax, 1 (sys_write)
        0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00,
        // 58: mov rdi, 1 (stdout)
        0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00,
        // 65: lea rsi, [rip+53] (Buffer) -> RIP=72. 72+53=125.
        0x48, 0x8d, 0x35, 0x35, 0x00, 0x00, 0x00,
        // 72: mov rdx, r13
        0x4c, 0x89, 0xea,
        // 75: syscall
        0x0f, 0x05,
        
        // 77: mov rax, 3 (sys_close)
        0x48, 0xc7, 0xc0, 0x03, 0x00, 0x00, 0x00,
        // 84: mov rdi, r12
        0x4c, 0x89, 0xe7,
        // 87: syscall
        0x0f, 0x05,
        
        // 89: mov rax, 60 (sys_exit)
        0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,
        // 96: xor rdi, rdi
        0x48, 0x31, 0xff,
        // 99: syscall
        0x0f, 0x05,
        
        // 101: Padding
        0x90, 0x90, 0x90,
        
        // 104: Path "/etc/hostname\0"
        b'/', b'e', b't', b'c', b'/', b'h', b'o', b's', b't', b'n', b'a', b'm', b'e', 0x00,
        
        // 118: Padding
        0,0,0,0,0,0,0,
        
        // 125: Buffer (32 bytes)
        0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
    ];
}