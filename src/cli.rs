use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Opts {
    #[structopt(name = "DEVICE", short = "p", long = "device", parse(from_os_str))]
    /// Device with BTRFS partition (ex: /dev/sda3)
    pub device: PathBuf,

    /// Use ficlonerange call
    #[structopt(short = "m", long = "fast-mode")]
    pub fast_mode: bool,

    /// Show logs messages
    #[structopt(short = "v", long)]
    pub verbose: bool,

    /// Report deduplicate data status with different chunk size
    #[structopt(short = "a", long)]
    pub analyze: bool,

    /// Parse dir recursively (used along with -d)
    #[structopt(short = "r", long)]
    pub recurse: bool,

    /// Show summary of dedupe details
    #[structopt(short = "D", long = "dry-run")]
    pub dry_run: bool,

    /// Will skip backup/validation process.
    #[structopt(short = "s", long)]
    pub skip: bool,

    /// Dedupe chunk size in KB TODO: set default 128
    #[structopt(name = "CHUNK_SZ", short = "c", long = "chunk-size")]
    pub chunk_size: Option<i32>,

    /// Dedupe list of files
    #[structopt(
        name = "FILE",
        short = "f",
        long = "files",
        parse(from_os_str),
        conflicts_with = "DIR",
        required_unless = "DIR"
    )]
    pub files: Vec<PathBuf>,

    /// Dedupe given directory or directories
    #[structopt(
        name = "DIR",
        short = "d",
        long = "dir",
        parse(from_os_str),
        conflicts_with = "FILE"
    )]
    pub dir_path: Vec<PathBuf>,
}
