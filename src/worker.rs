use rayon::prelude::*;
use std::time::Instant;

use crate::converter::*;

pub struct Worker {
    queue: Vec<String>,
    threads: usize,
    from: RegionFormat,
    to: RegionFormat,
}

impl Worker {
    pub fn new(queue: Vec<String>, threads: usize, from: RegionFormat, to: RegionFormat) -> Self {
        Self {
            queue,
            threads,
            from,
            to,
        }
    }

    pub fn run(&self) {
        let start = Instant::now();

        self.queue.par_iter().for_each(|file| {
            let region = match self.from {
                RegionFormat::Anvil => todo!(),
                RegionFormat::LinearV1 => todo!(),
                RegionFormat::LinearV2 => todo!(),
                _ => (),
            };
        });

        let end = Instant::now();
        let time = end - start;
    }
}
