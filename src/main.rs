use std::path::PathBuf;

use clap::{Parser, Subcommand};
use converter::RegionFormat;

mod converter;
mod error;
mod worker;

/**
 * linear-tools
 *      Commands:
 *          linear - Converts from anvil to linear format
 *          anvil  - Converts from linear to anvil format
 *          verify - Verifies a directory of linear files
 *
 *      Required:
 *          <src> - Source file or directory
 *
 *      Optional:
 *          --compression-level - The zstd compression level to use for linear conversions
 *          --workers - Number of worker processes to use (Default to core count * 2)
 *          --metrics - Provides metrics and progress report
 *          --output  - Output processed regions to another directory
 */

#[derive(Parser)]
#[command(name = "linear-tools")]
struct Cli {
    #[arg(short, long, help = "The source file or directory to process files")]
    input: PathBuf,

    #[arg(
        short,
        long,
        help = "Specifies the number of worker threads for conversion"
    )]
    workers: Option<u8>,

    #[arg(
        short = 'm',
        long = "track-metrics",
        help = "Provides metrics and status updates"
    )]
    track_metrics: bool,
}

#[derive(Subcommand)]
enum Command {
    Convert {
        from: RegionFormat,
        to: RegionFormat,
        output: Option<PathBuf>,
    },
    Verify,
}

fn main() {
    let cli = Cli::parse();
}
