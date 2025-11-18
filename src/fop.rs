use itertools::Itertools;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

pub fn btrfs_dump_csum(filename: &PathBuf, device_name: PathBuf) -> String {
    let btrfs_bin = "/usr/sbin/btrfs.static";

    let output = Command::new(btrfs_bin)
        .arg("inspect-internal")
        .arg("dump-csum")
        .arg(filename)
        .arg(&device_name)
        .output()
        .expect("failed to execute process");

    println!("status: {}", output.status);
    //println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn do_dedupe(src_file: &PathBuf, dst_file: &PathBuf, dry_run: bool, device: PathBuf) -> bool {
    let out1 = btrfs_dump_csum(src_file, device.clone());
    println!("{}",out1);
    let out2 = btrfs_dump_csum(dst_file, device.clone());
    println!("{}",out2);

    true
}

pub fn validate_file(filename: &PathBuf) -> Result<&PathBuf, io::Error> {
    let stat = fs::metadata(filename)?;
    let file_type = stat.file_type();

    match file_type.is_file() {
        true => return Ok(filename),
        false => return Err(io::Error::new(io::ErrorKind::Other, "Not a regular file")),
    };
}

pub fn validate_files(src_file: &PathBuf, dst_file: &PathBuf) -> Result<bool, io::Error> {
    let src_stat = fs::metadata(src_file)?;
    let s_file_type = src_stat.file_type();

    let dst_stat = fs::metadata(dst_file)?;
    let d_file_type = dst_stat.file_type();

    match (s_file_type.is_file(), d_file_type.is_file()) {
        (true, true) => return Ok(true),
        _ => return Err(io::Error::new(io::ErrorKind::Other, "Not a regular file")),
    };
}

pub fn dedupe_files(files_list: Vec<PathBuf>, device: PathBuf, dry_run: bool) {
    if dry_run {
        println!("dry run mode");
    }
    for file in &files_list {
        // TODO: Print error message neatly
        match validate_file(file) {
            Ok(file) => println!("{:#?} is a regular file.", file),
            Err(error) => println!("{:#?} Error: {}", file, error),
        };
    }
    let comb: Vec<Vec<&PathBuf>> = files_list.iter().combinations(2).collect::<Vec<_>>();

    for f in &comb {
        match validate_files(f[0], f[1]) {
            Ok(_) => {
                println!(" {:#?} {:#?} are valid files.", f[0], f[1]);
                do_dedupe(f[0], f[1], dry_run, device.clone());
            }
            Err(error) => println!("Error: {}", error),
        };
    }
}

pub fn dedupe_dir(dir_path: Vec<PathBuf>, device: PathBuf, dry_run: bool, recurse: bool) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = Vec::new();
    if dry_run {
        println!("dry run mode");
    }
    if recurse {
        println!("recurse mode");
    }
    for dir in &dir_path {
        entries = fs::read_dir(dir)?
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, io::Error>>()?;
    }
    for f in &entries {
        println!("{:#?}", f);
    }
    dedupe_files(entries, device.clone(),dry_run);
    Ok(())
}
