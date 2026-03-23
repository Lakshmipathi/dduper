use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "dduper",
    version = "0.05",
    about = "BTRFS block-level dedup tool"
)]
pub struct Opts {
    /// Device with BTRFS partition (ex: /dev/sda3)
    #[arg(short = 'p', long = "device", required = true)]
    pub device: PathBuf,

    /// Dedupe list of files
    #[arg(short = 'f', long = "files", num_args = 1.., conflicts_with = "dir_path")]
    pub files: Vec<PathBuf>,

    /// Dedupe given directory or directories
    #[arg(short = 'd', long = "dir", num_args = 1.., conflicts_with = "files")]
    pub dir_path: Vec<PathBuf>,

    /// Parse dir recursively (used along with -d)
    #[arg(short = 'r', long = "recurse")]
    pub recurse: bool,

    /// Show summary of dedupe details
    #[arg(short = 'D', long = "dry-run")]
    pub dry_run: bool,

    /// Will skip backup/validation process
    #[arg(short = 's', long = "skip")]
    pub skip: bool,

    /// Dedupe chunk size in KB (default: 128, must be multiple of 4)
    #[arg(short = 'c', long = "chunk-size", default_value = "128")]
    pub chunk_size: u64,

    /// Use ficlonerange call (fast mode)
    #[arg(short = 'm', long = "fast-mode")]
    pub fast_mode: bool,

    /// Show log messages
    #[arg(short = 'V', long = "verbose")]
    pub verbose: bool,

    /// Find perfect match files only
    #[arg(short = 'P', long = "perfect-match-only")]
    pub perfect_match_only: bool,

    /// Report deduplicate data status with different chunk sizes
    #[arg(short = 'a', long = "analyze")]
    pub analyze: bool,
}
