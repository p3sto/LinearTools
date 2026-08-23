use crate::{converter::*, error};

use std::{
    cmp,
    fs::{self, OpenOptions},
    io::{BufWriter, Cursor, Read, Write},
    ops::Deref,
    time::SystemTime,
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use error::Result;
use zstd::encode_all;

const LINEAR_SIGNATURE: u64 = 0xc3ff13183cca9d9a;
const LINEAR_HEADER_SIZE: usize = MAX_REGION_CHUNKS * 8;
const LINEAR_SUPERBLOCK_SIZE: usize = 32;

struct LinearSuperblock {
    signature: u64,
    version: u8,
    newest_timestamp: u64,
    compression_level: i8,
    chunk_count: i16,
    compressed_len: u32,
    reserved: u64,
}

impl LinearSuperblock {
    fn read_bytes(bytes: &[u8]) -> Result<Self> {
        let mut bytes = bytes;
        if bytes.len() < LINEAR_SUPERBLOCK_SIZE {
            return Err("Invalid superblock size".into());
        }

        let signature = bytes.read_u64::<BigEndian>()?;
        let version = bytes.read_u8()?;
        let newest_timestamp = bytes.read_u64::<BigEndian>()?;
        let compression_level = bytes.read_i8()?;
        let chunk_count = bytes.read_i16::<BigEndian>()?;
        let compressed_len = bytes.read_u32::<BigEndian>()?;
        let reserved = bytes.read_u64::<BigEndian>()?;

        Ok(Self {
            signature,
            version,
            newest_timestamp,
            compression_level,
            chunk_count,
            compressed_len,
            reserved,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::with_capacity(LINEAR_SUPERBLOCK_SIZE);
        buffer.extend_from_slice(&self.signature.to_be_bytes());
        buffer.push(self.version);
        buffer.extend_from_slice(&self.newest_timestamp.to_be_bytes());
        buffer.extend_from_slice(&self.compression_level.to_be_bytes());
        buffer.extend_from_slice(&self.chunk_count.to_be_bytes());
        buffer.extend_from_slice(&self.compressed_len.to_be_bytes());
        buffer.extend_from_slice(&self.reserved.to_be_bytes());
        buffer
    }
}

#[derive(Clone, Copy, Default)]
struct ChunkHeader {
    size: u32,
    timestamp: u32,
}
struct ChunkHeaders([ChunkHeader; MAX_REGION_CHUNKS]);

impl Default for ChunkHeaders {
    fn default() -> Self {
        Self([ChunkHeader::default(); MAX_REGION_CHUNKS])
    }
}

impl Deref for ChunkHeaders {
    type Target = [ChunkHeader; MAX_REGION_CHUNKS];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct LinearChunkData {
    headers: ChunkHeaders,
    chunks: RegionChunks,
}

impl LinearChunkData {
    fn read_bytes(
        bytes: &[u8],
        superblock: &LinearSuperblock,
        region_x: i32,
        region_z: i32,
    ) -> Result<Self> {
        // Decompress this PHAT chunk of bytes
        let decompressed = zstd::decode_all(bytes)?;
        let decompressed_len = decompressed.len();

        let mut cursor = Cursor::new(decompressed);
        if cursor.get_ref().len() < LINEAR_HEADER_SIZE {
            return Err("Insufficient bytes to read chunk data headers".into());
        }

        // Extract chunk headers
        let mut chunk_headers = ChunkHeaders::default();
        let mut chunk_count = 0;
        let mut total_size = 0;
        for i in 0..MAX_REGION_CHUNKS {
            let header = &mut chunk_headers.0[i];
            header.size = cursor.read_u32::<BigEndian>()?;
            header.timestamp = cursor.read_u32::<BigEndian>()?;
            total_size += header.size;
            if header.size > 0 {
                chunk_count += 1;
            }
        }

        // Verify that chunk count matches superblock
        if chunk_count != superblock.chunk_count {
            return Err("Chunk count invalid".into());
        }

        // Verify size of chunk data
        if total_size != (LINEAR_HEADER_SIZE + decompressed_len) as u32 {
            return Err("Invalid decompression size".into());
        }

        // Extract region chunks
        let mut region_chunks = RegionChunks::default();
        for i in 0..MAX_REGION_CHUNKS {
            let size = chunk_headers.0[i].size;
            if size > 0 {
                let mut chunk_data = vec![0u8; size as usize];
                let x = REGION_DIMENSION * region_x + (i as i32) % REGION_DIMENSION;
                let z = REGION_DIMENSION * region_z + (i as i32) / REGION_DIMENSION;
                cursor.read_exact(&mut chunk_data)?;
                region_chunks.0[i] = Some(Chunk::new(x, z, &chunk_data));
            }
        }

        Ok(Self {
            headers: chunk_headers,
            chunks: region_chunks,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::new();
        for header in &self.headers.0 {
            buffer.extend_from_slice(&header.size.to_be_bytes());
            buffer.extend_from_slice(&header.timestamp.to_be_bytes());
        }

        for chunk in &self.chunks.0 {
            match chunk {
                Some(c) => {
                    buffer.extend_from_slice(&c.x.to_be_bytes());
                    buffer.extend_from_slice(&c.z.to_be_bytes());
                }
                None => continue,
            }
        }

        buffer
    }
}

struct LinearFooter(u64);

impl LinearFooter {
    fn read_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 {
            return Err("Unsifficient byte length to read footer".into());
        }

        let footer = u64::from_be_bytes(bytes.try_into()?);
        Ok(LinearFooter(footer))
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::with_capacity(8);
        buffer.extend_from_slice(&self.0.to_be_bytes());
        buffer
    }
}
struct LinearRegionFile {
    superblock: LinearSuperblock,
    chunk_data: LinearChunkData,
    footer: LinearFooter,
}

impl LinearRegionFile {
    fn read_bytes(bytes: &[u8], region_x: i32, region_z: i32) -> Result<LinearRegionFile> {
        let len = bytes.len();
        let superblock = LinearSuperblock::read_bytes(&bytes[0..32])?;
        let chunk_data =
            LinearChunkData::read_bytes(&bytes[33..len - 8], &superblock, region_x, region_z)?;
        let footer = LinearFooter::read_bytes(&bytes[(len - 8)..])?;

        if superblock.signature != LINEAR_SIGNATURE {
            return Err("Invalid header signature".into());
        }

        if footer.0 != LINEAR_SIGNATURE {
            return Err("Invalid footer signature".into());
        }

        Ok(LinearRegionFile {
            superblock,
            chunk_data,
            footer,
        })
    }

    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&self.superblock.to_bytes());
        bytes.extend_from_slice(&self.chunk_data.to_bytes());
        bytes.extend_from_slice(&self.footer.to_bytes());
        Ok(bytes)
    }
}

struct LinearV1Converter;
impl LinearV1Converter {
    fn verify(superblock: &LinearSuperblock, footer: &LinearFooter) -> Result<()> {
        if superblock.signature != LINEAR_SIGNATURE {
            return Err("Invalid header signature for region file".into());
        }

        if superblock.version != 1 {
            return Err(format!(
                "Invalid version number (expected 1, found: {})",
                superblock.version
            )
            .into());
        }

        if footer.0 != LINEAR_SIGNATURE {
            return Err("Invalid footer signature for region file".into());
        }

        Ok(())
    }

    fn verify_region(path: &str) -> Result<()> {
        let buffer = Self::read_to_buffer(path)?;
        let superblock = LinearSuperblock::read_bytes(&buffer[0..32])?;
        let footer = LinearFooter::read_bytes(&buffer[buffer.len() - 8..])?;
        Self::verify(&superblock, &footer)
    }
}

impl RegionConverter for LinearV1Converter {
    fn open_region_file(path: &str) -> Result<Region> {
        let buffer = Self::read_to_buffer(&path)?;
        let (region_x, region_z) = Self::parse_region_coords(path)?;
        let region_file = LinearRegionFile::read_bytes(&buffer, region_x, region_z)?;

        if region_file.superblock.signature != LINEAR_SIGNATURE {
            return Err("Invalid header signature for region file".into());
        }

        if region_file.footer.0 != LINEAR_SIGNATURE {
            return Err("Invalid footer signature for region file".into());
        }

        let modified_time: u32 = fs::metadata(&path)?
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs() as u32;

        let timestamps: [u32; MAX_REGION_CHUNKS] = region_file
            .chunk_data
            .headers
            .map(|header| header.timestamp);

        Ok(Region::new(
            region_file.chunk_data.chunks,
            region_x,
            region_z,
            modified_time,
            timestamps,
        ))
    }

    fn write_region_file(region: Region, output_dir: &str, compression_level: i8) -> Result<()> {
        let output_file = format!("{}/r.{}.{}.linear", output_dir, region.x, region.z);
        let wip_file = format!("{}/r.{}.{}.linear.wip", output_dir, region.x, region.z);
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&wip_file)?;

        let mut newest_timestamp = 0;
        let mut chunk_count = 0;
        let mut inner: Vec<u8> = Vec::new();
        for i in 0..MAX_REGION_CHUNKS {
            if let Some(chunk) = &region.chunks.0[i] {
                let size = chunk.chunk_data.len() as u32;
                let timestamp = region.timestamps[i] as u64;
                inner.extend_from_slice(&size.to_be_bytes());
                inner.extend_from_slice(&timestamp.to_be_bytes());
                chunk_count += 1;
                newest_timestamp = cmp::max(timestamp, newest_timestamp);
            } else {
                inner.extend_from_slice(&0u32.to_be_bytes());
                inner.extend_from_slice(&0u32.to_be_bytes());
            }
        }

        for i in 0..MAX_REGION_CHUNKS {
            if let Some(chunk) = &region.chunks.0[i] {
                inner.extend_from_slice(chunk.chunk_data.as_slice());
            }
        }

        let encoded = encode_all(inner.as_slice(), compression_level as i32)?;
        let encoded_length = encoded.len() as u32;
        let mut buffer = BufWriter::new(file);

        // Write superblock
        buffer.write_u64::<BigEndian>(LINEAR_SIGNATURE)?;
        buffer.write_u8(1)?;
        buffer.write_u64::<BigEndian>(newest_timestamp)?;
        buffer.write_i8(compression_level)?;
        buffer.write_i16::<BigEndian>(chunk_count)?;
        buffer.write_u32::<BigEndian>(encoded_length)?;
        buffer.write_u64::<BigEndian>(0)?; // Reserved (unused hash)
        buffer.write_all(&encoded)?;
        buffer.write_u64::<BigEndian>(LINEAR_SIGNATURE)?;

        buffer.flush()?;
        fs::rename(wip_file, output_file)?;
        Ok(())
    }
}
