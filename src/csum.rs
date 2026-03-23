use anyhow::{bail, Context, Result};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use crate::db::CsumDb;

// 4KB block size (BTRFS default)
pub const BLK_SIZE: u64 = 4;

// Matches hex values with or without 0x prefix
static CSUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:0x)?[0-9a-fA-F]{2,}").unwrap());

/// Parse btrfs dump-csum output to extract hex checksum values.
/// Handles both `0x1234abcd` and bare `1234abcd` formats.
pub fn parse_btrfs_csum_output(output: &str) -> Vec<String> {
    let mut csums = Vec::new();
    for line in output.lines() {
        for cap in CSUM_RE.find_iter(line) {
            csums.push(cap.as_str().to_string());
        }
    }
    csums
}

/// Compute SHA256 hash of checksum data (used as short_hash in DB)
pub fn compute_csum_hash(csums: &[String]) -> String {
    let mut hasher = Sha256::new();
    let repr = format!("{:?}", csums);
    hasher.update(repr.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Fetch BTRFS checksums directly (no cache)
fn do_btrfs_dump_csum(filename: &Path, device: &Path) -> Result<Vec<String>> {
    let btrfs_bin = if Path::new("/usr/sbin/btrfs.static").exists() {
        "/usr/sbin/btrfs.static"
    } else {
        "btrfs"
    };

    let output = Command::new(btrfs_bin)
        .arg("inspect-internal")
        .arg("dump-csum")
        .arg(filename)
        .arg(device)
        .output()
        .context("Failed to execute btrfs command")?;

    if !output.status.success() {
        bail!(
            "btrfs dump-csum failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_btrfs_csum_output(&stdout))
}

/// Fetch checksums with DB caching
pub fn btrfs_dump_csum_cached(filename: &Path, device: &Path, db: &CsumDb) -> Result<Vec<String>> {
    let fname = filename.to_string_lossy();

    // Check cache first
    if let Some(cached) = db.get_cached_csum(&fname)? {
        return Ok(parse_btrfs_csum_output(&cached));
    }

    // Cache miss: fetch from BTRFS
    let csums = do_btrfs_dump_csum(filename, device)?;

    // Store in DB
    let short_hash = compute_csum_hash(&csums);
    let csum_str = csums.join(" ");
    db.insert_csum(&fname, &short_hash, &csum_str)?;

    Ok(csums)
}

/// Fetch checksums without caching (for file mode without DB)
#[allow(dead_code)]
pub fn btrfs_dump_csum(filename: &Path, device: &Path) -> Result<Vec<String>> {
    do_btrfs_dump_csum(filename, device)
}

/// Group checksums into chunks and compute SHA256 hash for each chunk.
/// Returns (hash_map, collision_count) where hash_map maps SHA256 -> list of chunk offsets.
pub fn get_hashes(
    csums: &[String],
    ele_sz: usize,
    verbose: bool,
) -> (HashMap<String, Vec<usize>>, usize) {
    let mut hash_map: HashMap<String, Vec<usize>> = HashMap::new();
    let mut collision_count = 0;

    if ele_sz == 1 {
        for (idx, csum) in csums.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(csum.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            let entry = hash_map.entry(hash.clone()).or_default();
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
            let chunk_str = chunk
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("");
            let mut hasher = Sha256::new();
            hasher.update(chunk_str.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            let entry = hash_map.entry(hash.clone()).or_default();
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

/// Calculate element size based on chunk size in KB.
/// chunk_sz must be a positive multiple of BLK_SIZE (4KB).
pub fn get_ele_size(chunk_sz: u64) -> Result<usize> {
    if chunk_sz == 0 || !chunk_sz.is_multiple_of(BLK_SIZE) {
        bail!(
            "Ensure chunk size is a multiple of {}KB (e.g., {}, 8, 16, 32, 64, 128, 256)",
            BLK_SIZE,
            BLK_SIZE
        );
    }
    let no_of_csums = chunk_sz / BLK_SIZE;
    // For small chunk sizes (4KB-28KB), each csum is its own element
    let ele_sz = if no_of_csums < 8 {
        no_of_csums as usize
    } else {
        (no_of_csums / 8) as usize
    };
    // ele_sz must be at least 1
    Ok(ele_sz.max(1))
}

/// Auto-adjust chunk size based on file size for perfect matches.
/// Returns (adjusted_chunk_sz, ele_sz).
pub fn auto_adjust_chunk_sz(
    src_file_sz: u64,
    analyze: bool,
    current_chunk_sz: u64,
) -> (u64, usize) {
    if analyze {
        return (
            current_chunk_sz,
            get_ele_size(current_chunk_sz).unwrap_or(1),
        );
    }

    let fz_mb = src_file_sz >> 20;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_btrfs_csum_output_with_prefix() {
        let output = "0x12345678 0xabcdef01\n0xdeadbeef\n";
        let csums = parse_btrfs_csum_output(output);
        assert_eq!(csums, vec!["0x12345678", "0xabcdef01", "0xdeadbeef"]);
    }

    #[test]
    fn test_parse_btrfs_csum_output_bare_hex() {
        let output = "b9ad82f7 aef24db7 7d58f506 e8fdba47\n12345678\n";
        let csums = parse_btrfs_csum_output(output);
        assert_eq!(
            csums,
            vec!["b9ad82f7", "aef24db7", "7d58f506", "e8fdba47", "12345678"]
        );
    }

    #[test]
    fn test_parse_empty_output() {
        let csums = parse_btrfs_csum_output("");
        assert!(csums.is_empty());
    }

    #[test]
    fn test_get_ele_size_valid() {
        // Small chunk sizes: no_of_csums < 8, ele_sz = no_of_csums
        assert_eq!(get_ele_size(4).unwrap(), 1);   // 4/4=1 csum
        assert_eq!(get_ele_size(8).unwrap(), 2);   // 8/4=2 csums
        assert_eq!(get_ele_size(16).unwrap(), 4);  // 16/4=4 csums
        // Large chunk sizes: no_of_csums >= 8, ele_sz = no_of_csums/8
        assert_eq!(get_ele_size(128).unwrap(), 4);  // 128/4/8=4
        assert_eq!(get_ele_size(256).unwrap(), 8);
        assert_eq!(get_ele_size(512).unwrap(), 16);
        assert_eq!(get_ele_size(1024).unwrap(), 32);
    }

    #[test]
    fn test_get_ele_size_invalid() {
        assert!(get_ele_size(0).is_err());
        assert!(get_ele_size(3).is_err());   // not multiple of 4
        assert!(get_ele_size(5).is_err());   // not multiple of 4
        assert!(get_ele_size(127).is_err()); // not multiple of 4
    }

    #[test]
    fn test_auto_adjust_chunk_sz() {
        // 16MB+ file -> 16384KB chunk
        let (sz, _) = auto_adjust_chunk_sz(20 * 1024 * 1024, false, 128);
        assert_eq!(sz, 16384);

        // 1MB file -> 1024KB chunk
        let (sz, _) = auto_adjust_chunk_sz(1024 * 1024, false, 128);
        assert_eq!(sz, 1024);

        // 100KB file -> 128KB chunk
        let (sz, _) = auto_adjust_chunk_sz(100 * 1024, false, 128);
        assert_eq!(sz, 128);

        // Analyze mode: keep current chunk_sz
        let (sz, _) = auto_adjust_chunk_sz(20 * 1024 * 1024, true, 256);
        assert_eq!(sz, 256);
    }

    #[test]
    fn test_get_hashes_ele_sz_1() {
        let csums: Vec<String> = vec!["0xaa".into(), "0xbb".into(), "0xaa".into()];
        let (map, collisions) = get_hashes(&csums, 1, false);
        // 0xaa appears twice -> 1 collision
        assert_eq!(collisions, 1);
        // Should have 2 unique hashes
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_compute_csum_hash() {
        let csums = vec!["0x1234".to_string(), "0x5678".to_string()];
        let hash = compute_csum_hash(&csums);
        // Should be deterministic
        assert_eq!(hash, compute_csum_hash(&csums));
        // Should be different for different input
        let other = vec!["0xabcd".to_string()];
        assert_ne!(hash, compute_csum_hash(&other));
    }
}
