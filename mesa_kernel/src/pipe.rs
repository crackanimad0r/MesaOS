//! Pipes: buffer en kernel para comunicación entre procesos (cmd1 | cmd2)

use alloc::vec::Vec;
use spin::Mutex;

const PIPE_BUF_SIZE: usize = 4096;
const MAX_PIPES: usize = 16;

/// FD base para lectura de pipe (100, 101, ... 115)
pub const PIPE_READ_BASE: i32 = 100;
/// FD base para escritura de pipe (200, 201, ... 215)
pub const PIPE_WRITE_BASE: i32 = 200;

/// Buffer circular para un pipe
struct PipeBuffer {
    data: [u8; PIPE_BUF_SIZE],
    read_pos: usize,
    write_pos: usize,
    len: usize,
    /// Writer cerró → no más datos
    write_closed: bool,
}

impl PipeBuffer {
    fn new() -> Self {
        Self {
            data: [0; PIPE_BUF_SIZE],
            read_pos: 0,
            write_pos: 0,
            len: 0,
            write_closed: false,
        }
    }

    fn write(&mut self, buf: &[u8]) -> usize {
        if self.write_closed {
            return 0;
        }
        let mut written = 0;
        for &byte in buf {
            if self.len >= PIPE_BUF_SIZE {
                break;
            }
            self.data[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % PIPE_BUF_SIZE;
            self.len += 1;
            written += 1;
        }
        written
    }

    fn read(&mut self, buf: &mut [u8]) -> usize {
        let mut n = 0;
        while n < buf.len() && self.len > 0 {
            buf[n] = self.data[self.read_pos];
            self.read_pos = (self.read_pos + 1) % PIPE_BUF_SIZE;
            self.len -= 1;
            n += 1;
        }
        n
    }

    fn available(&self) -> usize {
        self.len
    }

    fn close_write(&mut self) {
        self.write_closed = true;
    }

    fn is_read_eof(&self) -> bool {
        self.write_closed && self.len == 0
    }
}

static PIPES: Mutex<[Option<Mutex<PipeBuffer>>; MAX_PIPES]> = Mutex::new([
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
]);

/// Crea un pipe. Retorna (read_fd, write_fd) o error.
pub fn create_pipe() -> Result<(i32, i32), &'static str> {
    let mut pipes = PIPES.lock();
    for id in 0..MAX_PIPES {
        if pipes[id].is_none() {
            pipes[id] = Some(Mutex::new(PipeBuffer::new()));
            let read_fd = PIPE_READ_BASE + id as i32;
            let write_fd = PIPE_WRITE_BASE + id as i32;
            crate::serial_println!("[PIPE] Created pipe {} -> read_fd={}, write_fd={}", id, read_fd, write_fd);
            return Ok((read_fd, write_fd));
        }
    }
    Err("Too many pipes")
}

/// Escribe en el FD de escritura de un pipe. Retorna bytes escritos o error.
pub fn pipe_write(fd: i32, buf: &[u8]) -> Result<usize, i32> {
    if fd < PIPE_WRITE_BASE || fd >= PIPE_WRITE_BASE + MAX_PIPES as i32 {
        return Err(-9); // EBADF
    }
    let id = (fd - PIPE_WRITE_BASE) as usize;
    let pipes = PIPES.lock();
    if let Some(ref pipe) = pipes[id] {
        Ok(pipe.lock().write(buf))
    } else {
        Err(-9)
    }
}

/// Lee del FD de lectura de un pipe. Retorna bytes leídos, 0 = EOF, negativo = error.
pub fn pipe_read(fd: i32, buf: &mut [u8]) -> Result<usize, i32> {
    if fd < PIPE_READ_BASE || fd >= PIPE_READ_BASE + MAX_PIPES as i32 {
        return Err(-9);
    }
    let id = (fd - PIPE_READ_BASE) as usize;
    let pipes = PIPES.lock();
    if let Some(ref pipe) = pipes[id] {
        let n = pipe.lock().read(buf);
        Ok(n)
    } else {
        Err(-9)
    }
}

/// Cierra el extremo de escritura del pipe (para que read devuelva 0 al vaciar).
pub fn pipe_close_write(fd: i32) -> Result<(), i32> {
    if fd < PIPE_WRITE_BASE || fd >= PIPE_WRITE_BASE + MAX_PIPES as i32 {
        return Err(-9);
    }
    let id = (fd - PIPE_WRITE_BASE) as usize;
    let pipes = PIPES.lock();
    if let Some(ref pipe) = pipes[id] {
        pipe.lock().close_write();
        Ok(())
    } else {
        Err(-9)
    }
}

/// Devuelve true si fd es un FD de pipe (lectura o escritura).
pub fn is_pipe_fd(fd: i32) -> bool {
    (fd >= PIPE_READ_BASE && fd < PIPE_READ_BASE + MAX_PIPES as i32)
        || (fd >= PIPE_WRITE_BASE && fd < PIPE_WRITE_BASE + MAX_PIPES as i32)
}
