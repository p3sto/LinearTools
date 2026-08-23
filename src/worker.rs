use rayon::prelude::*;
use std::time::Instant;

use crate::converter::*;

pub struct Worker {
    queue: Vec<String>,
    threads: usize,
    from: RegionType,
    to: RegionType,
}

impl Worker {
    pub fn new(queue: Vec<String>, threads: usize, from: RegionType, to: RegionType) -> Self {
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
                RegionType::Anvil => todo!(),
                RegionType::LinearV1 => todo!(),
                RegionType::LinearV2 => todo!(),
                _ => (),
            };
        });

        let end = Instant::now();
        let time = end - start;
    }
}
