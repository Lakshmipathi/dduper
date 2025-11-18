use itertools::Itertools;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::process::Command;

// Constants
const BLK_SIZE: u64 = 4; // 4KB block size
const FICLONERANGE: u64 = 0x4020940d;
const FIDEDUPERANGE: u64 = 0xc0189436;

// Global state for tracking processed files
static mut PROCESSED_FILES: Vec<PathBuf> = Vec::new();

#[repr(C)]
#[derive(Debug)]
struct FileCloneRange {
    src_fd: i64,
    src_offset: u64,
    src_length: u64,
    dest_offset: u64,
}

#[repr(C)]
#[derive(Debug)]
struct FileDedupeRange {
    src_offset: u64,
    src_length: u64,
    dest_count: u16,
    reserved1: u16,
    reserved2: u32,
    info: FileDedupeRangeInfo,
}

#[repr(C)]
#[derive(Debug)]
struct FileDedupeRangeInfo {
    dest_fd: i64,
    dest_offset: u64,
    bytes_deduped: u64,
    status: i32,
    reserved: u32,
}

pub struct DedupeStats {
    pub chunk_size: u64,
    pub src_chunks: usize,
    pub dst_chunks: usize,
    pub matched_chunks: usize,
    pub unmatched_chunks: usize,
    pub total_bytes_deduped: u64,
    pub perfect_match: bool,
}

/// Parse btrfs dump-csum output to extract checksums
fn parse_btrfs_csum_output(output: &str) -> Vec<String> {
    let mut csums = Vec::new();
    let re = Regex::new(r"0x[0-9a-fA-F]+").unwrap();

    for line in output.lines() {
        for cap in re.find_iter(line) {
            csums.push(cap.as_str().to_string());
        }
    }

    csums
}

/// Dump checksums from BTRFS for a file
pub fn btrfs_dump_csum(filename: &PathBuf, device_name: &PathBuf) -> io::Result<Vec<String>> {
    let btrfs_bin = if std::path::Path::new("/usr/sbin/btrfs.static").exists() {
        "/usr/sbin/btrfs.static"
    } else {
        "btrfs"
    };

    let output = Command::new(btrfs_bin)
        .arg("inspect-internal")
        .arg("dump-csum")
        .arg(filename)
        .arg(device_name)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("btrfs command failed: {}", String::from_utf8_lossy(&output.stderr)),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_btrfs_csum_output(&stdout))
}

/// Group checksums into chunks and compute their hashes
fn get_hashes(csums: &[String], ele_sz: usize, verbose: bool) -> (HashMap<String, Vec<usize>>, usize) {
    let mut hash_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut collision_count = 0;

    if ele_sz == 1 {
        for (idx, csum) in csums.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(csum.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            let entry = hash_map.entry(hash.clone()).or_insert_with(Vec::new);
            if !entry.is_empty() {
                if verbose {
                    println!("Collision with: {} at offset: {}", hash, idx);
                }
                collision_count += 1;
            }
            entry.push(idx);
        }
    } else {
        for (idx, chunk) in csums.chunks(ele_sz).enumerate() {
            let chunk_str = chunk.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("");
            let mut hasher = Sha256::new();
            hasher.update(chunk_str.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            let entry = hash_map.entry(hash.clone()).or_insert_with(Vec::new);
            if !entry.is_empty() {
                if verbose {
                    println!("Collision with: {} at offset: {}", hash, idx);
                }
                collision_count += 1;
            }
            entry.push(idx);
        }
    }

    (hash_map, collision_count)
}

/// Calculate element size based on chunk size
fn get_ele_size(chunk_sz: u64) -> io::Result<usize> {
    if chunk_sz <= 0 || chunk_sz % 128 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Ensure chunk size is a multiple of 128KB (128, 256, 512, etc.)",
        ));
    }

    let no_of_chunks = chunk_sz / BLK_SIZE;
    let ele_sz = (no_of_chunks / 8) as usize;
    Ok(ele_sz)
}

/// Auto-adjust chunk size based on file size
fn auto_adjust_chunk_sz(src_file_sz: u64, analyze: bool, current_chunk_sz: u64) -> (u64, usize) {
    if analyze {
        return (current_chunk_sz, get_ele_size(current_chunk_sz).unwrap_or(1));
    }

    let fz_mb = src_file_sz >> 20; // Convert to MB

    let perfect_match_chunk_sz = if fz_mb >= 16 {
        16384
    } else if fz_mb >= 8 {
        8192
    } else if fz_mb >= 4 {
        4096
    } else if fz_mb >= 2 {
        2048
    } else if fz_mb >= 1 {
        1024
    } else if (src_file_sz >> 10) >= 512 {
        512
    } else {
        128
    };

    let ele_sz = get_ele_size(perfect_match_chunk_sz).unwrap_or(1);
    (perfect_match_chunk_sz, ele_sz)
}

/// Perform ioctl FICLONERANGE
unsafe fn ioctl_ficlonerange(dst_fd: i32, range: &FileCloneRange) -> io::Result<()> {
    let ret = libc::ioctl(dst_fd, FICLONERANGE, range as *const FileCloneRange);
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Perform ioctl FIDEDUPERANGE
unsafe fn ioctl_fideduperange(src_fd: i32, range: &mut FileDedupeRange) -> io::Result<(u64, i32)> {
    let ret = libc::ioctl(src_fd, FIDEDUPERANGE, range as *mut FileDedupeRange);
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((range.info.bytes_deduped, range.info.status))
}

/// Calculate SHA256 checksum of a file
fn sha256_file(path: &PathBuf) -> io::Result<String> {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "sha256sum failed"));
    }

    let result = String::from_utf8_lossy(&output.stdout);
    Ok(result.split_whitespace().next().unwrap_or("").to_string())
}

/// Compare two files for equality
fn cmp_files(file1: &PathBuf, file2: &PathBuf) -> io::Result<bool> {
    let hash1 = sha256_file(file1)?;
    let hash2 = sha256_file(file2)?;
    Ok(hash1 == hash2)
}

/// Validate deduplication results
fn validate_results(src_file: &PathBuf, dst_file: &PathBuf, bkup_file: &PathBuf) -> io::Result<()> {
    if cmp_files(dst_file, bkup_file)? {
        println!("Dedupe validation successful {}:{}", src_file.display(), dst_file.display());
        fs::remove_file(bkup_file)?;
    } else {
        let msg = format!(
            "\nFAILURE: Deduplication for {} resulted in corruption. You can restore original file from {}",
            dst_file.display(),
            bkup_file.display()
        );
        println!("{}", msg);

        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/var/log/dduper_backupfile_info.log")?;
        writeln!(log, "{}", msg)?;
    }

    Ok(())
}

/// Display summary of deduplication
fn display_summary(stats: &DedupeStats, src_file: &PathBuf, dst_file: &PathBuf,
                   _dst_file_sz: u64, dry_run: bool, analyze: bool) {
    if analyze {
        print!("[Analyzing] {}:{}                   \r",
               src_file.display(), dst_file.display());
        io::stdout().flush().unwrap();
        return;
    }

    println!("Summary");
    println!("blk_size: {}KB  chunksize: {}KB", BLK_SIZE, stats.chunk_size);
    println!("{} has {} chunks", src_file.display(), stats.src_chunks);
    println!("{} has {} chunks", dst_file.display(), stats.dst_chunks);
    println!("Matched chunks: {}", stats.matched_chunks);
    println!("Unmatched chunks: {}", stats.unmatched_chunks);

    let avail_dedupe = stats.matched_chunks as u64 * stats.chunk_size;

    if dry_run {
        println!("Total size(KB) available for dedupe: {}", avail_dedupe);
    } else {
        println!("Total size(KB) deduped: {}", stats.total_bytes_deduped / 1024);
    }
}

/// Main deduplication logic
pub fn do_dedupe(
    src_file: &PathBuf,
    dst_file: &PathBuf,
    device: &PathBuf,
    dry_run: bool,
    skip: bool,
    fast_mode: bool,
    verbose: bool,
    analyze: bool,
    chunk_sz: u64,
) -> io::Result<DedupeStats> {
    let src_file_sz = fs::metadata(src_file)?.len();
    let dst_file_sz = fs::metadata(dst_file)?.len();

    let bkup_file = PathBuf::from(format!("{}.__dduper", dst_file.display()));

    // Dump checksums
    let src_csums = btrfs_dump_csum(src_file, device)?;
    let dst_csums = btrfs_dump_csum(dst_file, device)?;

    if src_csums.is_empty() || dst_csums.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "Empty checksums"));
    }

    // Check for perfect match
    let perfect_match = src_csums == dst_csums;
    let (actual_chunk_sz, ele_sz) = if perfect_match {
        println!("Perfect match: {} {}", src_file.display(), dst_file.display());
        auto_adjust_chunk_sz(src_file_sz, analyze, chunk_sz)
    } else {
        (chunk_sz, get_ele_size(chunk_sz)?)
    };

    // Get hashes
    let (src_dict, src_ccount) = get_hashes(&src_csums, ele_sz, verbose);
    let (dst_dict, dst_ccount) = if perfect_match {
        (src_dict.clone(), src_ccount)
    } else {
        get_hashes(&dst_csums, ele_sz, verbose)
    };

    // Find matched and unmatched keys
    let src_keys: HashSet<_> = src_dict.keys().collect();
    let dst_keys: HashSet<_> = dst_dict.keys().collect();
    let matched_keys: Vec<_> = src_keys.intersection(&dst_keys).collect();
    let unmatched_keys: Vec<_> = dst_keys.difference(&src_keys).collect();

    // Calculate matched and unmatched chunks
    let mut matched_chunks = 0;
    for key in &matched_keys {
        if let Some(offsets) = dst_dict.get(**key) {
            matched_chunks += offsets.len();
        }
    }

    let mut unmatched_chunks = 0;
    for key in &unmatched_keys {
        if let Some(offsets) = dst_dict.get(**key) {
            unmatched_chunks += offsets.len();
        }
    }

    let mut total_bytes_deduped = 0u64;
    let no_of_chunks = actual_chunk_sz / BLK_SIZE;
    let src_len = no_of_chunks * BLK_SIZE * 1024;

    if !dry_run {
        // Create backup
        if !skip {
            Command::new("cp")
                .arg("--reflink=always")
                .arg(dst_file)
                .arg(&bkup_file)
                .output()?;
        }

        let src_fd = fs::File::open(src_file)?;
        let dst_fd = fs::OpenOptions::new().write(true).open(dst_file)?;
        let src_fd_raw = src_fd.as_raw_fd();
        let dst_fd_raw = dst_fd.as_raw_fd();

        println!("{}", "*".repeat(24));

        for key in &matched_keys {
            if let Some(src_offsets) = src_dict.get(**key) {
                if let Some(dst_offsets) = dst_dict.get(**key) {
                    let src_offset = src_offsets[0] as u64 * src_len;

                    for &dst_idx in dst_offsets {
                        let dst_offset = dst_idx as u64 * src_len;

                        // Adjust length for final chunk
                        let actual_len = if src_offsets[0] == (src_dict.len() - 1) {
                            src_file_sz - src_offset
                        } else {
                            src_len
                        };

                        unsafe {
                            if fast_mode {
                                let range = FileCloneRange {
                                    src_fd: src_fd_raw as i64,
                                    src_offset,
                                    src_length: actual_len,
                                    dest_offset: dst_offset,
                                };
                                if let Err(e) = ioctl_ficlonerange(dst_fd_raw, &range) {
                                    eprintln!("ioctl_ficlonerange error: {}", e);
                                } else {
                                    total_bytes_deduped += actual_len;
                                }
                            } else {
                                let mut range = FileDedupeRange {
                                    src_offset,
                                    src_length: actual_len,
                                    dest_count: 1,
                                    reserved1: 0,
                                    reserved2: 0,
                                    info: FileDedupeRangeInfo {
                                        dest_fd: dst_fd_raw as i64,
                                        dest_offset: dst_offset,
                                        bytes_deduped: 0,
                                        status: 0,
                                        reserved: 0,
                                    },
                                };

                                if let Ok((bytes_dup, _status)) = ioctl_fideduperange(src_fd_raw, &mut range) {
                                    total_bytes_deduped += bytes_dup;
                                }
                            }
                        }
                    }
                }
            }
        }

        drop(src_fd);
        drop(dst_fd);

        println!("Dedupe completed for {}:{}", src_file.display(), dst_file.display());

        // Validate results
        if !skip {
            validate_results(src_file, dst_file, &bkup_file)?;
        }
    }

    let stats = DedupeStats {
        chunk_size: actual_chunk_sz,
        src_chunks: src_dict.len() + src_ccount,
        dst_chunks: dst_dict.len() + dst_ccount,
        matched_chunks,
        unmatched_chunks,
        total_bytes_deduped,
        perfect_match: perfect_match || (matched_chunks as u64 * actual_chunk_sz * 1024 == dst_file_sz),
    };

    display_summary(&stats, src_file, dst_file, dst_file_sz, dry_run, analyze);

    Ok(stats)
}

/// Validate a single file
pub fn validate_file(filename: &PathBuf) -> Result<&PathBuf, io::Error> {
    let stat = fs::metadata(filename)?;
    let file_type = stat.file_type();

    if file_type.is_file() && stat.len() >= 4096 {
        Ok(filename)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Not a regular file or file size < 4KB",
        ))
    }
}

/// Validate two files for deduplication
pub fn validate_files(src_file: &PathBuf, dst_file: &PathBuf) -> Result<bool, io::Error> {
    let src_stat = fs::metadata(src_file)?;
    let dst_stat = fs::metadata(dst_file)?;

    unsafe {
        if PROCESSED_FILES.contains(src_file) || PROCESSED_FILES.contains(dst_file) {
            return Ok(false);
        }
    }

    if src_stat.file_type().is_file()
        && dst_stat.file_type().is_file()
        && src_stat.ino() != dst_stat.ino()
        && src_stat.len() >= 4096
        && dst_stat.len() >= 4096
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Deduplicate a list of files
pub fn dedupe_files(
    files_list: Vec<PathBuf>,
    device: PathBuf,
    dry_run: bool,
    skip: bool,
    fast_mode: bool,
    verbose: bool,
    analyze: bool,
    chunk_sz: u64,
) {
    if dry_run {
        println!("Dry run mode");
    }

    // Validate all files first
    for file in &files_list {
        match validate_file(file) {
            Ok(_) => {
                if verbose {
                    println!("{:?} is a regular file.", file);
                }
            }
            Err(error) => println!("{:?} Error: {}", file, error),
        }
    }

    // Generate all combinations of files
    let combinations: Vec<Vec<&PathBuf>> = files_list.iter().combinations(2).collect();

    for pair in &combinations {
        if pair.len() == 2 {
            match validate_files(pair[0], pair[1]) {
                Ok(true) => {
                    if verbose {
                        println!("{:?} {:?} are valid files.", pair[0], pair[1]);
                    }
                    match do_dedupe(
                        pair[0],
                        pair[1],
                        &device,
                        dry_run,
                        skip,
                        fast_mode,
                        verbose,
                        analyze,
                        chunk_sz,
                    ) {
                        Ok(stats) => {
                            if stats.perfect_match {
                                unsafe {
                                    PROCESSED_FILES.push(pair[1].clone());
                                }
                            }
                        }
                        Err(e) => eprintln!("Deduplication error: {}", e),
                    }
                }
                Ok(false) => {
                    if verbose {
                        println!("Skipping {:?} {:?}", pair[0], pair[1]);
                    }
                }
                Err(error) => println!("Error: {}", error),
            }
        }
    }
}

/// Deduplicate files in a directory
pub fn dedupe_dir(
    dir_path: Vec<PathBuf>,
    device: PathBuf,
    dry_run: bool,
    recurse: bool,
    skip: bool,
    fast_mode: bool,
    verbose: bool,
    analyze: bool,
    chunk_sz: u64,
) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = Vec::new();

    if dry_run {
        println!("Dry run mode");
    }
    if recurse {
        println!("Recurse mode");
        for dir in &dir_path {
            fn walk_dir(dir: &PathBuf, entries: &mut Vec<PathBuf>) -> io::Result<()> {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() {
                        if let Ok(_) = validate_file(&path) {
                            entries.push(path);
                        }
                    } else if path.is_dir() {
                        walk_dir(&path, entries)?;
                    }
                }
                Ok(())
            }
            walk_dir(dir, &mut entries)?;
        }
    } else {
        for dir in &dir_path {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Ok(_) = validate_file(&path) {
                        entries.push(path);
                    }
                }
            }
        }
    }

    if entries.len() < 2 {
        println!("Single file given or empty directory. Try again with --recurse");
        return Ok(());
    }

    dedupe_files(entries, device, dry_run, skip, fast_mode, verbose, analyze, chunk_sz);
    Ok(())
}
