use std::{
    cmp::max,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Cursor, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use zstd::{encode_all, zstd_safe::InBuffer};

const REGION_DIMENSION: usize = 32;
const COMPRESSION_TYPE_ZLIB: u8 = 2;
const EXTERNAL_FILE_COMPRESSION_TYPE: u8 = 130; // 128 + 2
const LINEAR_SIGNATURE: u64 = 0xc3ff13183cca9d9a;
const SUPPORTED_VERSION: [u8; 2] = [1, 2];
const HEADER_SIZE: usize = 8192;
const CHUNK_HEADER_SIZE: usize = 1024;

type ChunksList = Vec<Option<Chunk>>;

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
    chunks: ChunksList,
    x: i32,
    z: i32,
    modified_time: SystemTime,
    timestamps: Vec<u32>,
}
impl Region {
    fn new(chunks: ChunksList, x: i32, z: i32, mtime: SystemTime, t: &[u32]) -> Self {
        Self {
            chunks,
            x,
            z,
            modified_time: mtime,
            timestamps: t.to_vec(),
        }
    }
}

trait Converter {
    fn open_region_file(path: &str) -> Result<Region>;
    fn write_region_file(region: Region, output: &str, compression: i8) -> Result<()>;

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
            let mut file = File::open(path).map_err(|_| anyhow!("Failed to open file: {path}"))?;
            let mut buffer: Vec<u8> = Vec::new();
            file.read_to_end(&mut buffer);
            Ok(buffer)
        } else {
            Err(anyhow!("File does not exist"))
        }
    }
}

struct LinearV1Converter;
impl LinearV1Converter {
    fn verify_files(path: PathBuf) {
        todo!()
    }

    fn extract_chunks(
        region_x: i32,
        region_z: i32,
        chunk_data: &[u8],
        expected_chunk_count: u16,
    ) -> Result<(ChunksList, Vec<u32>)> {
        let region_data: Vec<u8> = zstd::decode_all(chunk_data)
            .map_err(|e| anyhow!("Zstd decompression failed: {}", e))?;
        let region_len = region_data.len();

        let mut timestamps: Vec<u32> = Vec::with_capacity(CHUNK_HEADER_SIZE);
        let mut sizes: Vec<u32> = Vec::with_capacity(CHUNK_HEADER_SIZE);
        let mut actual_chunk_count = 0;
        let mut total_size = 0;

        // Extract chunk sizes and timestamps
        let mut header_cursor = Cursor::new(&region_data);
        for i in 0..CHUNK_HEADER_SIZE {
            if header_cursor.position() as usize + 8 > region_len {
                return Err(anyhow!(
                    "Chunk header size is out of bounds at record {}",
                    i
                ));
            }

            let size = header_cursor
                .read_u32::<BigEndian>()
                .map_err(|_| anyhow!("Failed to read chunk size for chunk {i}"))?;
            let timestamp = header_cursor
                .read_u32::<BigEndian>()
                .map_err(|_| anyhow!("Failed to read chunk timestamp for chunk {i}"))?;

            timestamps.push(timestamp);
            sizes.push(size);
            total_size += size;

            if size != 0 {
                actual_chunk_count += 1;
            }
        }

        let total = total_size + HEADER_SIZE as u32;
        if total != (region_len as u32) {
            return Err(anyhow!(
                "Decompressed size is invalid: expected {total} bytes, got {region_len}"
            ));
        }

        if actual_chunk_count != expected_chunk_count {
            return Err(anyhow!(
                "Chunk count is invalid: expected {expected_chunk_count} got {actual_chunk_count}"
            ));
        }

        // Extract actual chunk data
        let mut chunks: ChunksList = vec![None; CHUNK_HEADER_SIZE];
        let mut data_cursor = Cursor::new(&region_data[HEADER_SIZE..]);
        for i in 0..CHUNK_HEADER_SIZE {
            let size = sizes[i] as usize;
            if size > 0 {
                let pos = data_cursor.position() as usize;
                if pos.saturating_add(size) > (region_len - HEADER_SIZE) {
                    return Err(anyhow!("Not enough data for chunk {i}"));
                }

                let mut data = vec![0u8; size];
                let iter = i as i32;
                let x = 32 * region_x + (iter % 32);
                let z = 32 * region_z + (iter / 32);
                data_cursor
                    .read_exact(&mut data)
                    .map_err(|_| anyhow!("Failed to read chunk data for chunk {i}"))?;
                chunks[i] = Some(Chunk::new(x, z, &data));
            }
        }

        Ok((chunks, timestamps))
    }
}

impl Converter for LinearV1Converter {
    fn open_region_file(path: &str) -> Result<Region> {
        let mut file: File =
            File::open(&path).map_err(|_| anyhow!("Could not open file at '{}'", path))?;
        let mut buffer: Vec<u8> = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mut cursor = Cursor::new(&buffer);
        let signature_begin = cursor.read_u64::<BigEndian>()?;
        let version = cursor.read_u8()?;
        cursor.seek(SeekFrom::Current(5))?; // Skip newest_timestamp (4) + compression_level (1)
        let chunk_count = cursor.read_u16::<BigEndian>()?;
        let compressed_length = cursor.read_u32::<BigEndian>();
        cursor.seek(SeekFrom::Current(8))?; // Skip

        cursor.seek(SeekFrom::End(-8))?;
        let signature_end = cursor.read_u64::<BigEndian>()?;

        // Check valid signature_begin
        if signature_begin != LINEAR_SIGNATURE {
            return Err(anyhow!("Invalid header signature for region file"));
        }

        // Check valid versions
        if !SUPPORTED_VERSION.contains(&version) {
            return Err(anyhow!("Invalid version number, found: '{version}'"));
        }

        // Check valid signature_end
        if signature_end != LINEAR_SIGNATURE {
            return Err(anyhow!("Invalid footer signature for region file"));
        }

        let compressed_data = &buffer[32..buffer.len() - 8];
        let (region_x, region_z) = Self::extract_region_coords(&path)?;
        let (chunks, timestamps) =
            Self::extract_chunks(region_x, region_z, compressed_data, chunk_count)?;
        let modified_time: SystemTime = file.metadata()?.modified()?;

        Ok(Region::new(
            chunks,
            region_x,
            region_z,
            modified_time,
            &timestamps,
        ))
    }

    fn write_region_file(region: Region, output_dir: &str, compression_level: i8) -> Result<()> {
        let output_file = format!("{output_dir}/r.{}.{}.linear", region.x, region.z);
        let wip_file = format!("{output_dir}/r.{}.{}.linear.wip", region.x, region.z);
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&wip_file)?;

        let mut newest_timestamp = 0;
        let mut chunk_count = 0;
        let mut inner: Vec<u8> = Vec::new();
        for i in 0..1024 {
            if let Some(chunk) = region.chunks.get(i).unwrap() {
                let size = chunk.chunk_data.len() as u32;
                let timestamp = region.timestamps[i] as u64;
                inner.extend_from_slice(&size.to_be_bytes());
                inner.extend_from_slice(&timestamp.to_be_bytes());
                chunk_count += 1;
                newest_timestamp = max(timestamp, newest_timestamp);
            } else {
                inner.extend_from_slice(&0u64.to_be_bytes());
            }
        }

        for i in 0..1024 {
            if let Some(chunk) = region.chunks.get(i).unwrap() {
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
        std::fs::rename(wip_file, output_file)?;
        Ok(())
    }
}

struct McaConverter;
impl Converter for McaConverter {
    fn open_region_file(path: &str) -> Result<Region> {
        let sector = 4096;

        let chunk_starts: Vec<u8> = Vec::new();
        let chunk_sizes: Vec<u8> = Vec::new();
        let timestamps: Vec<u8> = Vec::new();
        let chunks: Vec<u8> = Vec::new();

        let (region_x, region_z) = Self::extract_region_coords(path)?;
        for i in 0..1024 {}

        Err(anyhow!("Error"))
    }

    fn write_region_file(region: Region, output: &str, compression: i8) -> Result<()> {
        todo!()
    }
}
