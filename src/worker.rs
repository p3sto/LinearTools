use rayon::prelude::*;
use std::time::Instant;

use crate::converter::*;

pub struct Worker {
    queue: Vec<String>,
    threads: usize,
    from: ConverterType,
    to: ConverterType,
}

impl Worker {
    pub fn new(queue: Vec<String>, threads: usize, from: ConverterType, to: ConverterType) -> Self {
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
                ConverterType::Anvil => todo!(),
                ConverterType::LinearV1(_) => todo!(),
                _ => (),
            };
        });

        let end = Instant::now();
        let time = end - start;
    }
}
