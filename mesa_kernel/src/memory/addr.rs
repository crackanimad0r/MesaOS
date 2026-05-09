// mesa_kernel/src/memory/addr.rs

//! Tipos de direcciones de memoria

use core::fmt;

/// Dirección física (frame de memoria real)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(u64);

/// Dirección virtual (espacio de direcciones del proceso)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(u64);

impl PhysAddr {
    #[inline]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }
    
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        self.0 % align == 0
    }
    
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        Self(self.0 & !(align - 1))
    }
    
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        Self((self.0 + align - 1) & !(align - 1))
    }
}

impl VirtAddr {
    #[inline]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }
    
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    
    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }
    
    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
}

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PhysAddr({:#x})", self.0)
    }
}

impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VirtAddr({:#x})", self.0)
    }
}

impl From<u64> for PhysAddr {
    fn from(addr: u64) -> Self {
        Self(addr)
    }
}

impl From<u64> for VirtAddr {
    fn from(addr: u64) -> Self {
        Self(addr)
    }
}