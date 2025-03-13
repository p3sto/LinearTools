use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod converter;
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
    #[arg(long, help="Converts from anvil to linear format")]
    mca: bool,

    #[arg(long, )]
    linear: bool,

}

enum Command {
    
}

fn main() {
    let cli = Cli::parse();

    println!("Linear? {:?}", cli.linear);
    println!("Mca? {:?}", cli.mca);
}
