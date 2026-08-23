use crate::error;
use error::Result;
use std::{fs::OpenOptions, io::Read, ops::Deref, vec::IntoIter};

mod anvil;
mod linear_v1;
mod linear_v2;

const REGION_DIMENSION: i32 = 32;
const MAX_REGION_CHUNKS: usize = 1024;

#[derive(Clone)]
struct Chunk {
    x: i32,
    z: i32,
    chunk_data: Vec<u8>,
}

impl Chunk {
    fn new(x: i32, z: i32, data: &[u8]) -> Self {
        Chunk {
            x,
            z,
            chunk_data: data.to_vec(),
        }
    }
}

struct RegionChunks(Vec<Option<Chunk>>);

impl Default for RegionChunks {
    fn default() -> Self {
        Self(vec![None; MAX_REGION_CHUNKS])
    }
}

impl IntoIterator for RegionChunks {
    type Item = Option<Chunk>;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Deref for RegionChunks {
    type Target = Vec<Option<Chunk>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
struct Region {
    chunks: RegionChunks,
    x: i32,
    z: i32,
    modified_time: u32,
    timestamps: [u32; MAX_REGION_CHUNKS],
}

impl Region {
    fn new(
        chunks: RegionChunks,
        x: i32,
        z: i32,
        modified_time: u32,
        timestamps: [u32; MAX_REGION_CHUNKS],
    ) -> Self {
        Self {
            chunks,
            x,
            z,
            modified_time,
            timestamps,
        }
    }
}

#[derive(Clone)]
pub enum RegionType {
    Anvil,
    LinearV1,
    LinearV2,
}

pub trait RegionFormat<T> {
    fn from_bytes(&self, bytes: &[u8]) -> Result<T>;
    fn to_bytes(&self) -> Result<Vec<u8>>;
}

pub trait RegionConverter {
    fn open_region_file(path: &str) -> Result<Region>;
    fn write_region_file(region: Region, output: &str, compression: i8) -> Result<()>;

    /**
     * Extract the region file's coordinates from its path
     **/
    fn parse_region_coords(path: &str) -> Result<(i32, i32)> {
        let file_name = path
            .split("/")
            .last()
            .ok_or_else(|| "Could not extract filename from '{path}'")?;

        let tokens: Vec<&str> = file_name.split(".").collect();
        if tokens.len() != 4 {
            return Err("Region file is not named in the r.x.z.linear format".into());
        }

        let x: i32 = tokens
            .get(1)
            .ok_or_else(|| "Could not parse region_x")?
            .parse()?;
        let z: i32 = tokens
            .get(1)
            .ok_or_else(|| "Could not parse region_z")?
            .parse()?;

        Ok((x, z))
    }

    /*
     * Reads a file to buffer and returns the file buffer
     */
    fn read_to_buffer(path: &str) -> Result<Vec<u8>> {
        let mut file = OpenOptions::new().read(true).open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }
}
