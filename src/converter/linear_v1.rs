use crate::{
    converter::{
        Chunk, Region, RegionChunks, RegionConverter, MAX_REGION_CHUNKS, REGION_DIMENSION,
    },
    error,
};

use std::{
    char::MAX, cmp, error::Error, fs::OpenOptions, io::{BufWriter, Cursor, Read, Seek, SeekFrom}, ops::Deref, time::SystemTime
};

use byteorder::{BigEndian, ReadBytesExt};
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
    fn read(bytes: &[u8]) -> Result<Self> {
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
        superblock: LinearSuperblock,
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

        if chunk_count != superblock.chunk_count {
            return Err("Chunk count invalid".into());
        }

        if total_size != (LINEAR_HEADER_SIZE + decompressed_len) as u32 {
            return Err("Invalid decompression size".into());
        }

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

    fn to_bytes(&self) -> Result<&[u8]> {
        let mut bytes: Vec<u8> = Vec::new();
        for header in &self.headers.0 {
            bytes.extend_from_slice(&header.size.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp.to_be_bytes());
        }

        for chunk in &self.chunks.0 {
            match chunk {
                Some(c) => {
                    bytes.extend_from_slice(&c.x.to_be_bytes());
                    bytes.extend_from_slice(&c.z.to_be_bytes());
                }
                None => continue,
            }
        }

        Ok(bytes.)
    }
}

struct LinearFooter(u64);

impl LinearFooter {
    fn read_bytes(bytes: &[u8]) -> Result<Self> {
       let mut bytes = bytes;
       let val = bytes.read_u64::<BigEndian>()?; 
        Ok(LinearFooter(val))
    }
}

impl From<u64> for LinearFooter {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
struct LinearRegionFile {
    superblock: LinearSuperblock,
    chunk_data: LinearChunkData,
    footer: LinearFooter,
}

impl LinearRegionFile {
    fn read(bytes: &[u8], region_x: i32, region_z: i32) -> Result<LinearRegionFile> {
        let superblock = LinearSuperblock::read(&bytes[0..32])?;
        
        let footer = LinearFooter::from();

        if superblock.signature != LINEAR_SIGNATURE {
            return Err("Invalid header signature".into());
        }

        if footer != LINEAR_SIGNATURE {
            return Err("Invalid footer signature".into());
        }

        let chunk_data = LinearChunkData::read_bytes(
            &mut cursor,
            superblock.compressed_len,
            region_x,
            region_z,
        )?;
        let footer = LinearFooter(cursor.read_u8()?);
        Ok(LinearRegionFile {
            superblock,
            chunk_data,
            footer,
        })
    }

    fn to_bytes() -> Result<Vec<u8>> {
        let mut bytes: Vec<u8> = Vec::new();
        Ok(bytes)
    }
}

struct LinearV1Converter;
impl LinearV1Converter {
    fn verify_region(path: &str) -> Result<()> {
        let buffer = Self::read_to_buffer(path)?;

        let superblock = LinearSuperblock::read(&mut cursor)?;

        if superblock.signature_begin != LINEAR_SIGNATURE {
            return Err("Invalid header signature for region file");
        }

        if superblock.version != 1 {
            return Err("Invalid version number (expected 1, found: {version})");
        }

        if buf != LINEAR_SIGNATURE {
            return Err("Invalid footer signature for region file");
        }

        Ok(())
    }
}

impl RegionConverter for LinearV1Converter {
    fn open_region_file(path: &str) -> Result<Region> {
        let buffer = Self::read_to_buffer(&path)?;
        let (region_x, region_z) = Self::parse_region_coords(path)?;
        let region_file = LinearRegionFile::read_bytes(&buffer, region_x, region_z)?;

        if region_file.superblock.signature != LINEAR_SIGNATURE
            || region_file.footer.0 != LINEAR_SIGNATURE
        {
            return Err("Invalid footer signature for region file".into());
        }

        // Extract chunk sizes and timestamps from header
        let mut cursor = Cursor::new(&decoded);
        for i in 0..MAX_REGION_CHUNKS {
            let size = cursor
                .read_u32::<BigEndian>()
                .map_err(|_| "Failed to read chunk size for chunk {i}")?;
            let timestamp = cursor
                .read_u32::<BigEndian>()
                .map_err(|_| "Failed to read chunk timestamp for chunk {i}")?;

            timestamps.push(timestamp);
            sizes.push(size);
            total_size += size;

            if size != 0 {
                actual_chunk_count += 1;
            }
        }

        let total = total_size + LINEAR_HEADER_SIZE;
        if total != decoded_len {
            return Err(
                "Decompressed size is invalid: expected {total} bytes, got {decoded_len} bytes",
            );
        }

        if actual_chunk_count != chunk_count {
            return Err("Chunk count is invalid: expected {chunk_count}, got {actual_chunk_count}");
        }

        // Extract chunk data
        let mut chunks: Vec<Option<Chunk>> = vec![None; MAX_REGION_CHUNKS];
        for i in 0..MAX_REGION_CHUNKS {
            let size = sizes[i];
            if size > 0 {
                let mut data = vec![0u8; size as usize];
                let x = FILE_HEADER_SIZE * region_x + (i as i32) % FILE_HEADER_SIZE;
                let z = FILE_HEADER_SIZE * region_z + (i as i32) / FILE_HEADER_SIZE;
                cursor
                    .read_exact(&mut data)
                    .map_err(|_| "Failed to read chunk data for chunk {i}")?;
                chunks[i] = Some(Chunk::new(x, z, &data));
            }
        }

        let modified_time: SystemTime = fs::metadata(&path)?.modified()?;
        let region = Region::new(chunks, region_x, region_z, modified_time, &timestamps);
        Ok(region)
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
