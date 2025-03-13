use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::PathBuf,
    time::SystemTime,
};

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ReadBytesExt};

const REGION_DIMENSION: usize = 32;
const COMPRESSION_TYPE: u8 = b'\x02';
const COMPRESSION_TYPE_ZLIB: u8 = 2;
const EXTERNAL_FILE_COMPRESSION_TYPE: u8 = 130; // 128 + 2
const LINEAR_SIGNATURE: u64 = 0xc3ff13183cca9d9a;
const SUPPORTED_VERSION: [u8; 2] = [1, 2];
const LINEAR_VERSION: u8 = 1;
const HEADER_SIZE: usize = 8192;
const CHUNKS_PER_REGION: usize = 1024;

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
    fn open_region_file(path: PathBuf) -> Result<Region>;
    fn convert_region_file(output: PathBuf, region: Region, compression: u8) -> Result<()>;

    fn extract_region_coords(file_name: &str) -> Result<(i32, i32)> {
        let tokens: Vec<&str> = file_name.split(".").collect();
        if tokens.len() < 3 {
            return Err(anyhow!("Invalid region file name '{}'", file_name));
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
}

struct LinearConverter;
impl LinearConverter {
    fn verify_files(path: PathBuf) {
        todo!()
    }

    fn get_signature_end(cursor: &mut Cursor<Vec<u8>>) -> Result<u64> {
        let curr_pos = cursor.position();
        cursor.seek(SeekFrom::End(-8));
        let signature = cursor.read_u64::<BigEndian>()?;
        cursor.set_position(curr_pos);
        Ok(signature)
    }

    fn validate_region_header(buffer: &Vec<u8>) -> Result<u16> {
        let mut cursor = Cursor::new(buffer);

        let signature_begin = cursor.read_u64::<BigEndian>()?;
        let version = cursor.read_u8()?;
        cursor.seek(SeekFrom::Current(5)); // Skip newest timestamp + Compression Level
        let chunk_count = cursor.read_u16::<BigEndian>()?;
        cursor.seek(SeekFrom::Current(12)); // Skip complete_region_length and unsued hash (reserved)
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

        Ok(chunk_count)
    }

    fn extract_chunks(region_x:i32, region_z: i32, chunk_data: &[u8], expected_chunk_count: u16) -> Result<(ChunksList, Vec<u32>)> {
        let region_data: Vec<u8> = zstd::decode_all(chunk_data)
            .map_err(|e| anyhow!("Zstd decompression failed: {}", e))?;
        let region_len = region_data.len();

        let mut timestamps: Vec<u32> = Vec::with_capacity(CHUNKS_PER_REGION);
        let mut sizes: Vec<u32> = Vec::with_capacity(CHUNKS_PER_REGION);
        let mut actual_chunk_count = 0;
        let mut total_size = 0;

        // Extract chunk sizes and timestamps
        let mut header_cursor = Cursor::new(&region_data);
        for i in 0..CHUNKS_PER_REGION {
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
        let mut chunks: ChunksList = vec![None; CHUNKS_PER_REGION];
        let mut data_cursor = Cursor::new(&region_data[HEADER_SIZE..]);
        for i in 0..CHUNKS_PER_REGION {
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
                data_cursor.read_exact(&mut data).map_err(|_| anyhow!("Failed to read chunk data for chunk {i}"));
                chunks[i] = Some(Chunk::new(x, z, &data));
            }
        }

        Ok((chunks, timestamps))
    }
}

impl Converter for LinearConverter {
    fn open_region_file(path: PathBuf) -> Result<Region> {
        let file_name: &str = path
            .file_name()
            .ok_or_else(|| anyhow!("Could not retrieve file name"))?
            .to_str()
            .ok_or_else(|| anyhow!("File name contains invalid UTF-8 characters"))?;

        // Read file to buffer
        let (region_x, region_z) = Self::extract_region_coords(&file_name)?;
        let mut file: File =
            File::open(&path).context(anyhow!("Could not open file {}", file_name))?;
        let modified_time: SystemTime = file.metadata()?.modified()?;
        let mut buffer: Vec<u8> = Vec::new();
        file.read_to_end(&mut buffer);

        let zstd_data = &buffer[32..buffer.len() - 8]; 
        let expected_chunk_count = Self::validate_region_header(&buffer)?;
        let (chunks, timestamps) = Self::extract_chunks(region_x, region_z, zstd_data, expected_chunk_count)?;
        Ok(Region::new(chunks, region_x, region_z, modified_time, &timestamps))
    } 

    fn convert_region_file(output: PathBuf, region: Region, compression: u8) -> Result<()> {
        todo!()
    }
}

struct McaConverter;
impl Converter for McaConverter {
    fn open_region_file(path: PathBuf) -> Result<Region> {
        todo!()
    }

    fn convert_region_file(output: PathBuf, region: Region, compression: u8) -> Result<()> {
        todo!()
    }
}
