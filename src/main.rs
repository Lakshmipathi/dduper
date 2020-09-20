use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(name = "DEVICE", short = "p", long = "device", parse(from_os_str))]
    /// Device with BTRFS partition (ex: /dev/sda3)
    device: PathBuf,

    /// Use ficlonerange call
    #[structopt(short = "m", long = "fast-mode")]
    fast_mode: bool,

    /// Show logs messages
    #[structopt(short = "v", long)]
    verbose: bool,

    /// Report deduplicate data status with different chunk size
    #[structopt(short = "a", long)]
    analyze: bool,

    /// Parse dir recursively (used along with -d)
    #[structopt(short = "r", long)]
    recurse: bool,

    /// Show summary of dedupe details
    #[structopt(short = "D", long = "dry-run")]
    dry_run: bool,

    /// Will skip backup/validation process.
    #[structopt(short = "s", long)]
    skip: bool,

    /// Dedupe chunk size in KB TODO: set default 128
    #[structopt(name = "CHUNK_SZ", short = "c", long = "chunk-size")]
    chunk_size: Option<i32>,

    /// Dedupe list of files
    #[structopt(name = "FILE", short = "f", long = "files", parse(from_os_str))]
    files: Vec<PathBuf>,

    /// Dedupe given directory or directories
    #[structopt(
        name = "DIR",
        short = "d",
        long = "dir",
        parse(from_os_str),
        conflicts_with = "files"
    )]
    dir_path: Vec<PathBuf>,
}

fn main() {
    let opts = Opts::from_args();
    println!("opts: {:#?}", opts);
}
