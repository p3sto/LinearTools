#![allow(unused)]
use core::time;
use std::{
    fs::{self, File},
    io::{BufReader, Error, Read},
    path::{Path, PathBuf},
};

use zstd::stream::read;

const REGION_DIMENSION: usize = 32;
const COMPRESSION_TYPE: u8 = b'\x02';
const COMPRESSION_TYPE_ZLIB: u8 = 2;
const EXTERNAL_FILE_COMPRESSION_TYPE: u8 = 130; // 128 + 2
const LINEAR_SIGNATURE: i128 = 0xc3ff13183cca9d9a;
const SUPPORTED_VERSION: [u8; 2] = [1, 2];
const LINEAR_VERSION: u8 = 1;
const HEADER_SIZE: usize = REGION_DIMENSION * REGION_DIMENSION * 8;

type RawChunk = Vec<u8>;
type Coordinate = (i32, i32);

struct Chunk {
    coordinate: Coordinate,
    raw_chunk: RawChunk,
}

struct Region {
    chunks: Vec<Chunk>,
    coordinate: Coordinate,
    modified_time: u32,
    time_stamp: time::Duration,
}

trait Converter {
    fn read_region(path: PathBuf) -> Region;
    fn convert_region(output: PathBuf, region: Region, compression: u8);
}

struct LinearConverter;
impl LinearConverter {
    fn verify_files(path: PathBuf) {}
}

impl Converter for LinearConverter {
    fn read_region(path: PathBuf) -> Region {
        todo!()
    }

    fn convert_region(output: PathBuf, region: Region, compression: u8) {}
}

struct McaConverter;
impl Converter for McaConverter {
    fn read_region(path: PathBuf) -> Region {
        todo!()
    }

    fn convert_region(output: PathBuf, region: Region, compression: u8) {
        todo!()
    }
}
