use std::fs;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;
mod cli;

fn validate_file(filename: &PathBuf) -> Result<&PathBuf, io::Error> {
    let stat = fs::metadata(filename)?;
    let file_type = stat.file_type();

    match file_type.is_file() {
        true => return Ok(filename),
        _ => return Err(io::Error::new(io::ErrorKind::Other, "Not a regular file")),
    };
}

fn dedupe_files(files_list: Vec<PathBuf>, dry_run: bool) {
    for file in &files_list {
        // TODO: Print error message neatly
        match validate_file(file) {
            Ok(file) => println!("{:#?} is a regular file.", file),
            Err(error) => println!("{:#?} Error: {}", file, error),
        };
    }
}

fn dedupe_dir(dir_path: Vec<PathBuf>, dry_run: bool, recurse: bool) -> io::Result<()> {
    let mut entries = Vec::new();
    for dir in &dir_path {
        entries = fs::read_dir(dir)?
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, io::Error>>()?;
    }
    for f in &entries {
        println!("{:#?}", f);
    }
    dedupe_files(entries, true);
    Ok(())
}

fn main() {
    let opts = cli::Opts::from_args();
    dbg!(&opts);

    if opts.files.len() > 0 {
        dedupe_files(opts.files, opts.dry_run);
    } else {
        dedupe_dir(opts.dir_path, opts.dry_run, opts.recurse);
    }
}
