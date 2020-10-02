use std::fs;
use std::io;
use std::path::PathBuf;

pub fn validate_file(filename: &PathBuf) -> Result<&PathBuf, io::Error> {
    let stat = fs::metadata(filename)?;
    let file_type = stat.file_type();

    match file_type.is_file() {
        true => return Ok(filename),
        _ => return Err(io::Error::new(io::ErrorKind::Other, "Not a regular file")),
    };
}

pub fn dedupe_files(files_list: Vec<PathBuf>, dry_run: bool) {
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
}

pub fn dedupe_dir(dir_path: Vec<PathBuf>, dry_run: bool, recurse: bool) -> io::Result<()> {
    let mut entries = Vec::new();
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
    dedupe_files(entries, dry_run);
    Ok(())
}
