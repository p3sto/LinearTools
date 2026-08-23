use crate::{converter::REGION_DIMENSION, error::Result};

struct LinearSuperblock {
    signature: u64,
    version: u8,
    newest_timestamp: u64,
    grid_size: i8,
    region_x: i32,
    region_z: i32,
}

struct ChunkBitmap(u128);

impl ChunkBitmap {
    fn new(map: u128) -> Self {
        Self(map)
    }

    fn exists(&self, superblock: &LinearSuperblock, chunk_x: i32, chunk_z: i32) -> Result<bool> {
        let region_x = ((chunk_x / REGION_DIMENSION) as f32).floor() as i32;
        let region_z = ((chunk_z / REGION_DIMENSION) as f32).floor() as i32;
        if region_x != superblock.region_x {
            return Err(format!("Invalid x chunk for region {}", superblock.region_x).into());
        }

        if region_z != superblock.region_z {
            return Err(format!("Invalid z chunk for region {}", superblock.region_x).into());
        }

        if index & self.0 != 0 {}
        Ok(false)
    }
}
