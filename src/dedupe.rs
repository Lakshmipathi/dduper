use anyhow::{bail, Context, Result};
use itertools::Itertools;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use crate::csum;
use crate::db::CsumDb;

// ioctl constants
const FICLONERANGE: u64 = 0x4020940d;
const FIDEDUPERANGE: u64 = 0xc0189436;

// ioctl structs (must match kernel ABI)
#[repr(C)]
struct FileCloneRange {
    src_fd: i64,
    src_offset: u64,
    src_length: u64,
    dest_offset: u64,
}

#[repr(C)]
struct FileDedupeRange {
    src_offset: u64,
    src_length: u64,
    dest_count: u16,
    reserved1: u16,
    reserved2: u32,
    // Inline single dest_info entry
    dest_fd: i64,
    dest_offset: u64,
    bytes_deduped: u64,
    status: i32,
    reserved3: u32,
}

/// Configuration for a dedup operation
pub struct DedupeConfig {
    pub device: PathBuf,
    pub dry_run: bool,
    pub skip: bool,
    pub fast_mode: bool,
    pub verbose: bool,
    pub analyze: bool,
    pub perfect_match_only: bool,
    pub recurse: bool,
    pub chunk_sz: u64,
}

/// Entry in analyze results table
pub struct AnalyzeEntry {
    pub files: String,
    pub duplicate_kb: u64,
}

/// Session state for a dedup run (replaces global mutable state)
pub struct DedupeSession {
    pub processed_files: HashSet<PathBuf>,
    pub analyze_results: BTreeMap<u64, Vec<AnalyzeEntry>>,
    pub db: CsumDb,
}

impl DedupeSession {
    pub fn new(db: CsumDb) -> Self {
        DedupeSession {
            processed_files: HashSet::new(),
            analyze_results: BTreeMap::new(),
            db,
        }
    }
}

/// Stats from a single file-pair deduplication
#[allow(dead_code)]
pub struct DedupeStats {
    pub chunk_size: u64,
    pub src_chunks: usize,
    pub dst_chunks: usize,
    pub matched_chunks: usize,
    pub unmatched_chunks: usize,
    pub total_bytes_deduped: u64,
    pub perfect_match: bool,
    pub avail_dedupe_kb: u64,
}

// --- File validation ---

/// Validate a single file: must be a regular file >= 4KB
pub fn validate_file(path: &Path) -> Result<()> {
    let meta = fs::metadata(path).with_context(|| format!("Cannot stat {}", path.display()))?;
    if !meta.file_type().is_file() {
        bail!("{}: not a regular file", path.display());
    }
    if meta.len() < 4096 {
        bail!("{}: file size < 4KB", path.display());
    }
    Ok(())
}

/// Validate a pair of files for deduplication
pub fn validate_file_pair(src: &Path, dst: &Path, processed: &HashSet<PathBuf>) -> bool {
    if processed.contains(src) || processed.contains(dst) {
        return false;
    }

    let (src_stat, dst_stat) = match (fs::metadata(src), fs::metadata(dst)) {
        (Ok(s), Ok(d)) => (s, d),
        _ => return false,
    };

    src_stat.file_type().is_file()
        && dst_stat.file_type().is_file()
        && src_stat.ino() != dst_stat.ino()
        && src_stat.len() >= 4096
        && dst_stat.len() >= 4096
}

// --- SHA256 file comparison (in-process, no external sha256sum) ---

fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Validate dedup results by comparing dst with its backup
fn validate_results(src_file: &Path, dst_file: &Path, bkup_file: &Path) -> Result<()> {
    let dst_hash = sha256_file(dst_file)?;
    let bkup_hash = sha256_file(bkup_file)?;

    if dst_hash == bkup_hash {
        println!(
            "Dedupe validation successful {}:{}",
            src_file.display(),
            dst_file.display()
        );
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

// --- ioctl wrappers ---

/// Perform FICLONERANGE ioctl (fast mode)
/// # Safety: direct kernel ioctl call
unsafe fn ioctl_ficlonerange(dst_fd: i32, range: &FileCloneRange) -> io::Result<()> {
    let ret = libc::ioctl(dst_fd, FICLONERANGE, range as *const FileCloneRange);
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Perform FIDEDUPERANGE ioctl (safe mode)
/// # Safety: direct kernel ioctl call
unsafe fn ioctl_fideduperange(src_fd: i32, range: &mut FileDedupeRange) -> io::Result<(u64, i32)> {
    let ret = libc::ioctl(src_fd, FIDEDUPERANGE, range as *mut FileDedupeRange);
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((range.bytes_deduped, range.status))
}

// --- Core deduplication ---

/// Perform deduplication between two files
pub fn do_dedupe(
    src_file: &Path,
    dst_file: &Path,
    config: &DedupeConfig,
    session: &mut DedupeSession,
) -> Result<DedupeStats> {
    let src_file_sz = fs::metadata(src_file)?.len();
    let dst_file_sz = fs::metadata(dst_file)?.len();

    let bkup_file = PathBuf::from(format!("{}.__dduper", dst_file.display()));

    // Dump checksums (with caching)
    let src_csums = csum::btrfs_dump_csum_cached(src_file, &config.device, &session.db)?;
    let dst_csums = csum::btrfs_dump_csum_cached(dst_file, &config.device, &session.db)?;

    if src_csums.is_empty() || dst_csums.is_empty() {
        bail!(
            "Empty checksums for {}:{}",
            src_file.display(),
            dst_file.display()
        );
    }

    // Check for perfect match
    let perfect_match = src_csums == dst_csums;

    if perfect_match {
        println!(
            "Perfect match : {} {}",
            src_file.display(),
            dst_file.display()
        );
        if config.perfect_match_only {
            return Ok(DedupeStats {
                chunk_size: config.chunk_sz,
                src_chunks: 0,
                dst_chunks: 0,
                matched_chunks: 0,
                unmatched_chunks: 0,
                total_bytes_deduped: 0,
                perfect_match: true,
                avail_dedupe_kb: dst_file_sz / 1024,
            });
        }
    }

    let (actual_chunk_sz, ele_sz) = if perfect_match {
        csum::auto_adjust_chunk_sz(src_file_sz, config.analyze, config.chunk_sz)
    } else {
        (config.chunk_sz, csum::get_ele_size(config.chunk_sz)?)
    };

    // Get hashes
    let (src_dict, src_ccount) = csum::get_hashes(&src_csums, ele_sz, config.verbose);
    let (dst_dict, dst_ccount) = if perfect_match {
        (src_dict.clone(), src_ccount)
    } else {
        csum::get_hashes(&dst_csums, ele_sz, config.verbose)
    };

    // Find matched and unmatched keys
    let src_keys: HashSet<_> = src_dict.keys().collect();
    let dst_keys: HashSet<_> = dst_dict.keys().collect();

    let matched_keys: Vec<String> = src_keys
        .intersection(&dst_keys)
        .map(|k| (*k).clone())
        .collect();
    let unmatched_keys: Vec<String> = dst_keys
        .difference(&src_keys)
        .map(|k| (*k).clone())
        .collect();

    let matched_chunks: usize = matched_keys
        .iter()
        .filter_map(|k| dst_dict.get(k))
        .map(|v| v.len())
        .sum();
    let unmatched_chunks: usize = unmatched_keys
        .iter()
        .filter_map(|k| dst_dict.get(k))
        .map(|v| v.len())
        .sum();

    let mut total_bytes_deduped = 0u64;
    let no_of_chunks = actual_chunk_sz / csum::BLK_SIZE;
    let src_len = no_of_chunks * csum::BLK_SIZE * 1024;

    if !config.dry_run {
        // Create backup (fast mode only, unless skip)
        if !config.skip {
            let output = Command::new("cp")
                .arg("--reflink=always")
                .arg(dst_file)
                .arg(&bkup_file)
                .output()
                .context("Failed to create reflink backup")?;
            if !output.status.success() {
                log::warn!(
                    "Backup creation failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        let src_fd = fs::File::open(src_file)?;
        let dst_fd = fs::OpenOptions::new().write(true).open(dst_file)?;
        let src_fd_raw = src_fd.as_raw_fd();
        let dst_fd_raw = dst_fd.as_raw_fd();

        println!("{}", "*".repeat(24));

        for key in &matched_keys {
            if let (Some(src_offsets), Some(dst_offsets)) = (src_dict.get(key), dst_dict.get(key)) {
                let src_offset = src_offsets[0] as u64 * src_len;

                for &dst_idx in dst_offsets {
                    let dst_offset = dst_idx as u64 * src_len;

                    // Adjust length for final chunk
                    let actual_len = if src_offsets[0] == src_dict.len() - 1 {
                        src_file_sz - src_offset
                    } else {
                        src_len
                    };

                    // Safety: these are Linux kernel ioctls operating on valid file descriptors
                    unsafe {
                        if config.fast_mode {
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
                                dest_fd: dst_fd_raw as i64,
                                dest_offset: dst_offset,
                                bytes_deduped: 0,
                                status: 0,
                                reserved3: 0,
                            };

                            if let Ok((bytes_dup, _status)) =
                                ioctl_fideduperange(src_fd_raw, &mut range)
                            {
                                total_bytes_deduped += bytes_dup;
                            }
                        }
                    }
                }
            }
        }

        drop(src_fd);
        drop(dst_fd);

        println!(
            "Dedupe completed for {}:{}",
            src_file.display(),
            dst_file.display()
        );

        // Mark processed in DB
        session.db.mark_processed(&dst_file.to_string_lossy()).ok();

        // Validate results
        if !config.skip {
            validate_results(src_file, dst_file, &bkup_file)?;
        }
    }

    let avail_dedupe_kb = matched_chunks as u64 * actual_chunk_sz;
    let is_perfect = perfect_match || (avail_dedupe_kb * 1024 == dst_file_sz);

    let stats = DedupeStats {
        chunk_size: actual_chunk_sz,
        src_chunks: src_dict.len() + src_ccount,
        dst_chunks: dst_dict.len() + dst_ccount,
        matched_chunks,
        unmatched_chunks,
        total_bytes_deduped,
        perfect_match: is_perfect,
        avail_dedupe_kb,
    };

    // Display or collect results
    if config.analyze {
        eprint!(
            "[Analyzing] {}:{}                   \r",
            src_file.display(),
            dst_file.display()
        );
        io::stderr().flush().ok();

        let entry = AnalyzeEntry {
            files: format!("{}:{}", src_file.display(), dst_file.display()),
            duplicate_kb: if is_perfect {
                dst_file_sz / 1024
            } else {
                avail_dedupe_kb
            },
        };
        session
            .analyze_results
            .entry(config.chunk_sz)
            .or_default()
            .push(entry);
    } else {
        println!("Summary");
        println!(
            "blk_size: {}KB  chunksize: {}KB",
            csum::BLK_SIZE,
            actual_chunk_sz
        );
        println!("{} has {} chunks", src_file.display(), stats.src_chunks);
        println!("{} has {} chunks", dst_file.display(), stats.dst_chunks);
        println!("Matched chunks: {}", stats.matched_chunks);
        println!("Unmatched chunks: {}", stats.unmatched_chunks);

        if config.dry_run {
            println!("Total size(KB) available for dedupe: {}", avail_dedupe_kb);
        } else {
            println!("Total size(KB) deduped: {}", total_bytes_deduped / 1024);
        }
    }

    Ok(stats)
}

/// Deduplicate a list of files (pairwise combinations)
pub fn dedupe_files(
    files: &[PathBuf],
    config: &DedupeConfig,
    session: &mut DedupeSession,
) -> Result<()> {
    if files.len() < 2 {
        println!("Single file given or empty directory. Try again with --recurse");
        return Ok(());
    }

    if config.dry_run {
        println!("Dry run mode");
    }

    // Validate all files first
    for file in files {
        match validate_file(file) {
            Ok(_) => {
                if config.verbose {
                    println!("{:?} is a regular file.", file);
                }
            }
            Err(e) => println!("{:?} Error: {}", file, e),
        }
    }

    // Process all pairwise combinations
    for pair in files.iter().combinations(2) {
        let (src, dst) = (pair[0], pair[1]);

        if !validate_file_pair(src, dst, &session.processed_files) {
            if config.verbose {
                println!("Skipping {:?} {:?}", src, dst);
            }
            continue;
        }

        match do_dedupe(src, dst, config, session) {
            Ok(stats) => {
                if stats.perfect_match {
                    session.processed_files.insert(dst.clone());
                }
            }
            Err(e) => eprintln!("Deduplication error: {}", e),
        }
    }

    Ok(())
}

/// Multi-phase directory deduplication (matches Python's 4-phase approach)
pub fn dedupe_dir(
    dirs: &[PathBuf],
    config: &DedupeConfig,
    session: &mut DedupeSession,
) -> Result<()> {
    // Phase 1: Validate and collect files
    log::debug!("Phase-1: Validating files and creating DB");
    let file_list = collect_valid_files(dirs, config.recurse)?;

    if file_list.len() < 2 {
        println!("Single file given or empty directory. Try again with --recurse");
        return Ok(());
    }

    if config.dry_run {
        println!("Dry run mode");
    }
    if config.recurse {
        println!("Recurse mode");
    }

    // Phase 1.1: Populate checksums in DB for all files
    log::debug!("Phase-1.1: Populate records");
    for file in &file_list {
        csum::btrfs_dump_csum_cached(file, &config.device, &session.db)?;
        session.db.mark_valid(&file.to_string_lossy()).ok();
    }

    // Phase 2: Detect duplicate files via DB
    log::debug!("Phase-2: Detecting duplicate files");
    let dup_groups = session.db.detect_duplicates()?;

    // Phase 3: Dedupe duplicate file groups
    log::debug!("Phase-3: Dedupe duplicate files");
    for group in &dup_groups {
        let paths: Vec<PathBuf> = group.iter().map(PathBuf::from).collect();
        log::debug!("Deduping group: {:?}", paths);
        dedupe_files(&paths, config, session)?;
    }

    // Phase 4: Dedupe remaining unprocessed files
    log::debug!("Phase-4: Dedupe remaining files");
    let remaining = session.db.get_unprocessed()?;
    let remaining_paths: Vec<PathBuf> = remaining.iter().map(PathBuf::from).collect();
    log::debug!("Remaining files: {:?}", remaining_paths);
    dedupe_files(&remaining_paths, config, session)?;

    Ok(())
}

/// Walk directories and collect valid files
fn collect_valid_files(dirs: &[PathBuf], recurse: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for dir in dirs {
        if recurse {
            for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() && validate_file(entry.path()).is_ok() {
                    files.push(entry.into_path());
                }
            }
        } else {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && validate_file(&path).is_ok() {
                    files.push(path);
                }
            }
        }
    }

    Ok(files)
}
