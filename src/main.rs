use structopt::StructOpt;
mod cli;
mod fop;

fn main() {
    let opts = cli::Opts::from_args();
    dbg!(&opts);

    if opts.files.len() > 0 {
        fop::dedupe_files(opts.files, opts.device, opts.dry_run);
    } else {
        fop::dedupe_dir(opts.dir_path, opts.device, opts.dry_run, opts.recurse);
    }
}
