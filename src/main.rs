use std::path::PathBuf;
use structopt::StructOpt;
mod cli;

fn dedupe_files(files_list: Vec<PathBuf>, dry_run: bool) {
    todo!()
}

fn dedupe_dir(dir_path: Vec<PathBuf>, dry_run: bool, recurse: bool) {
    todo!()
}

fn main() {
    let opts = cli::Opts::from_args();
    dbg!(&opts);

    dedupe_files(opts.files, opts.dry_run);
    dedupe_dir(opts.dir_path, opts.dry_run, opts.recurse);
}
