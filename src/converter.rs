use std::{
    cmp, fs, io::{BufWriter, Cursor, Read, Seek, SeekFrom, Write}, path::Path, time::SystemTime
};

use anyhow::{anyhow, Context, Ok, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use zstd::{decode_all, encode_all};

const FILE_HEADER_SIZE: i32 = 32;
const LINEAR_SIGNATURE: u64 = 0xc3ff13183cca9d9a;
const CHUNK_RECORD_COUNT: usize = 1024;
const INTERNAL_HEADER_SIZE: u32 = CHUNK_RECORD_COUNT as u32 * 8;

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

struct Region {
    chunks: Vec<Option<Chunk>>,
    x: i32,
    z: i32,
    modified_time: SystemTime,
    timestamps: Vec<u32>,
}
impl Region {
    fn new(chunks: Vec<Option<Chunk>>, x: i32, z: i32, mtime: SystemTime, t: &[u32]) -> Self {
        Self {
            chunks,
            x,
            z,
            modified_time: mtime,
            timestamps: t.to_vec(),
        }
    }
}

#[derive(Clone)]
pub enum ConverterType {
    Anvil,
    LinearV1(u8),
}

pub trait Converter {
    fn open_region_file(path: &str) -> Result<Region>;
    fn write_region_file(region: Region, output: &str, compression: i8) -> Result<()>;
}

/**
 * Extract the region file's coordinates from its path
 **/
fn extract_region_coords(path: &str) -> Result<(i32, i32)> {
    let file_name = path
        .split("/")
        .last()
        .ok_or_else(|| anyhow!("Could not extract filename from '{path}'"))?;

    let tokens: Vec<&str> = file_name.split(".").collect();
    if tokens.len() != 4 {
        return Err(anyhow!(
            "Region file is not named in the r.{{x}}.{{z}}.linear format"
        ));
    }

    let x: i32 = tokens
        .get(1)
        .ok_or_else(|| anyhow!("Could not parse region_x"))?
        .parse()?;
    let z: i32 = tokens
        .get(1)
        .ok_or_else(|| anyhow!("Could not parse region_z"))?
        .parse()?;

    Ok((x, z))
}
/*
 * Reads a file to buffer and returns a
 */
fn read_to_buffer(path: &str) -> Result<Vec<u8>> {
    if fs::exists(path)? {
        let mut file = fs::OpenOptions::new()
            .open(path)
            .map_err(|e| anyhow!("Failed to open file '{path}': {e}"))?;
        let mut buffer: Vec<u8> = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    } else {
        Err(anyhow!("File does not exist"))
    }
}

struct LinearV1Converter;
impl LinearV1Converter {
    fn verify_region(path: &str) -> Result<()> {
        let mut file = fs::File::open(path)?;

        let mut header = vec![0u8; 9];
        file.read_exact(&mut header)
            .context(anyhow!("Failed to read header for verfication"))?;

        let signature_begin = u64::from_be_bytes(header[..8].try_into()?);
        let version = header[8];

        file.seek(SeekFrom::End(-8))?;
        let mut footer = vec![0u8; 8];
        file.read_exact(&mut footer)?;
        let signature_end = u64::from_be_bytes(footer[..].try_into()?);

        if signature_begin != LINEAR_SIGNATURE {
            return Err(anyhow!("Invalid header signature for region file"));
        }

        if version != 1 {
            return Err(anyhow!(
                "Invalid version number (expected 1, found: {version})"
            ));
        }

        if signature_end != LINEAR_SIGNATURE {
            return Err(anyhow!("Invalid footer signature for region file"));
        }

        Ok(())
    }
}

impl Converter for LinearV1Converter {
    fn open_region_file(path: &str) -> Result<Region> {
        let buffer = read_to_buffer(&path)?;
        let len = buffer.len();

        // Validate Superblock
        let signature_begin = u64::from_be_bytes(buffer[..8].try_into()?);
        let version = buffer[8];
        let chunk_count = i16::from_be_bytes(buffer[18..20].try_into()?);
        let expected_encoded_len = u32::from_be_bytes(buffer[20..24].try_into()?);
        let signature_end = u64::from_be_bytes(buffer[len - 8..].try_into()?);

        if signature_begin != LINEAR_SIGNATURE {
            return Err(anyhow!("Invalid header signature for region file"));
        }

        if version != 1 {
            return Err(anyhow!(
                "Invalid version number (expected 1, found: {version})"
            ));
        }

        if signature_end != LINEAR_SIGNATURE {
            return Err(anyhow!("Invalid footer signature for region file"));
        }

        // Extract encoded data
        let encoded = &buffer[FILE_HEADER_SIZE as usize..buffer.len() - 8];
        let encoded_len = encoded.len() as u32;
        if encoded_len as u32 != expected_encoded_len {
            return Err(anyhow!(
                "Compressed data length is invalid: expected {expected_encoded_len}, got {encoded_len}"
            ));
        }

        let (region_x, region_z) = extract_region_coords(&path)?;
        let decoded =
            decode_all(encoded).map_err(|e| anyhow!("Zstd decompression failed: {}", e))?;
        let decoded_len = decoded.len() as u32;
        let mut timestamps = Vec::with_capacity(CHUNK_RECORD_COUNT);
        let mut sizes = Vec::with_capacity(CHUNK_RECORD_COUNT);
        let mut actual_chunk_count = 0;
        let mut total_size = 0;

        // Extract chunk sizes and timestamps from header
        let mut cursor = Cursor::new(&decoded);
        for i in 0..CHUNK_RECORD_COUNT {
            let size = cursor
                .read_u32::<BigEndian>()
                .map_err(|_| anyhow!("Failed to read chunk size for chunk {i}"))?;
            let timestamp = cursor
                .read_u32::<BigEndian>()
                .map_err(|_| anyhow!("Failed to read chunk timestamp for chunk {i}"))?;

            timestamps.push(timestamp);
            sizes.push(size);
            total_size += size;

            if size != 0 {
                actual_chunk_count += 1;
            }
        }

        let total = total_size + INTERNAL_HEADER_SIZE;
        if total != decoded_len {
            return Err(anyhow!(
                "Decompressed size is invalid: expected {total} bytes, got {decoded_len} bytes"
            ));
        }

        if actual_chunk_count != chunk_count {
            return Err(anyhow!(
                "Chunk count is invalid: expected {chunk_count}, got {actual_chunk_count}"
            ));
        }

        // Extract chunk data
        let mut chunks: Vec<Option<Chunk>> = vec![None; CHUNK_RECORD_COUNT];
        for i in 0..CHUNK_RECORD_COUNT {
            let size = sizes[i];
            if size > 0 {
                let mut data = vec![0u8; size as usize];
                let x = FILE_HEADER_SIZE * region_x + (i as i32) % FILE_HEADER_SIZE;
                let z = FILE_HEADER_SIZE * region_z + (i as i32) / FILE_HEADER_SIZE;
                cursor
                    .read_exact(&mut data)
                    .map_err(|_| anyhow!("Failed to read chunk data for chunk {i}"))?;
                chunks[i] = Some(Chunk::new(x, z, &data));
            }
        }

        let modified_time: SystemTime = fs::metadata(&path)?.modified()?;
        let region = Region::new(chunks, region_x, region_z, modified_time, &timestamps);
        Ok(region)
    }

    fn write_region_file(region: Region, output_dir: &str, compression_level: i8) -> Result<()> {
        let output_file = format!("{output_dir}/r.{}.{}.linear", region.x, region.z);
        let wip_file = format!("{output_dir}/r.{}.{}.linear.wip", region.x, region.z);
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&wip_file)?;

        let mut newest_timestamp = 0;
        let mut chunk_count = 0;
        let mut inner: Vec<u8> = Vec::new();
        for i in 0..CHUNK_RECORD_COUNT {
            if let Some(chunk) = &region.chunks[i] {
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

        for i in 0..CHUNK_RECORD_COUNT {
            if let Some(chunk) = &region.chunks[i] {
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

#[derive(Default)]
pub struct AnvilConverter;
impl Converter for AnvilConverter {
    fn open_region_file(path: &str) -> Result<Region> {
        let sector = 4096;

        let chunk_starts: Vec<u8> = Vec::new();
        let chunk_sizes: Vec<u8> = Vec::new();
        let timestamps: Vec<u8> = Vec::new();
        let chunks: Vec<u8> = Vec::new();

        let (region_x, region_z) = extract_region_coords(path)?;
        for i in 0..CHUNK_RECORD_COUNT {}

        todo!()
    }

    fn write_region_file(region: Region, output: &str, compression: i8) -> Result<()> {
        todo!()
    }
}
