// mesa_kernel/src/drivers/block.rs

pub const SECTOR_SIZE: usize = 512;

pub trait BlockDevice: Send + Sync {
    fn read(&self, lba: u64, count: u16, buffer: &mut [u8]) -> Result<(), &'static str>;
    fn write(&self, lba: u64, count: u16, buffer: &[u8]) -> Result<(), &'static str>;
    fn capacity(&self) -> u64;
}
