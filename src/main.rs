use std::collections::HashMap;
use std::time::Instant;
use structopt::StructOpt;

mod cli;
mod fop;

fn main() {
    let mut opts = cli::Opts::from_args();
    let start = Instant::now();

    // Set skip to true when not using fast_mode
    let skip = if opts.fast_mode { opts.skip } else { true };

    // Set default chunk size if not specified
    let chunk_sz = opts.chunk_size.unwrap_or(128) as u64;

    if opts.analyze {
        // When analyze mode is enabled, also enable dry_run
        opts.dry_run = true;
        let mut analyze_dict: HashMap<u64, Vec<(String, u64)>> = HashMap::new();

        for sz in &[128u64, 256, 512, 1024, 2048, 4096, 8192, 16384] {
            // Run deduplication for each chunk size
            if !opts.files.is_empty() {
                run_dedupe_files(
                    &opts.files,
                    &opts.device,
                    opts.dry_run,
                    skip,
                    opts.fast_mode,
                    opts.verbose,
                    true,
                    *sz,
                    &mut analyze_dict,
                );
            } else if !opts.dir_path.is_empty() {
                run_dedupe_dir(
                    &opts.dir_path,
                    &opts.device,
                    opts.dry_run,
                    opts.recurse,
                    skip,
                    opts.fast_mode,
                    opts.verbose,
                    true,
                    *sz,
                    &mut analyze_dict,
                );
            }
        }

        // Display analysis results
        print_analysis_results(&analyze_dict);
    } else {
        // Normal mode
        if !opts.files.is_empty() {
            fop::dedupe_files(
                opts.files,
                opts.device,
                opts.dry_run,
                skip,
                opts.fast_mode,
                opts.verbose,
                false,
                chunk_sz,
            );
        } else if !opts.dir_path.is_empty() {
            if let Err(e) = fop::dedupe_dir(
                opts.dir_path,
                opts.device,
                opts.dry_run,
                opts.recurse,
                skip,
                opts.fast_mode,
                opts.verbose,
                false,
                chunk_sz,
            ) {
                eprintln!("Error: {}", e);
            }
        } else {
            eprintln!("No files or directories specified");
            std::process::exit(1);
        }
    }

    let duration = start.elapsed();
    println!("dduper took {:.2} seconds", duration.as_secs_f64());
}

fn run_dedupe_files(
    files: &[std::path::PathBuf],
    device: &std::path::PathBuf,
    dry_run: bool,
    skip: bool,
    fast_mode: bool,
    verbose: bool,
    analyze: bool,
    chunk_sz: u64,
    _analyze_dict: &mut HashMap<u64, Vec<(String, u64)>>,
) {
    // This is a stub for analyze mode - would need to collect stats
    // For now, just run the deduplication
    fop::dedupe_files(
        files.to_vec(),
        device.clone(),
        dry_run,
        skip,
        fast_mode,
        verbose,
        analyze,
        chunk_sz,
    );
}

fn run_dedupe_dir(
    dir_path: &[std::path::PathBuf],
    device: &std::path::PathBuf,
    dry_run: bool,
    recurse: bool,
    skip: bool,
    fast_mode: bool,
    verbose: bool,
    analyze: bool,
    chunk_sz: u64,
    _analyze_dict: &mut HashMap<u64, Vec<(String, u64)>>,
) {
    // This is a stub for analyze mode - would need to collect stats
    // For now, just run the deduplication
    if let Err(e) = fop::dedupe_dir(
        dir_path.to_vec(),
        device.clone(),
        dry_run,
        recurse,
        skip,
        fast_mode,
        verbose,
        analyze,
        chunk_sz,
    ) {
        eprintln!("Error in dedupe_dir: {}", e);
    }
}

fn print_analysis_results(analyze_dict: &HashMap<u64, Vec<(String, u64)>>) {
    use prettytable::{Cell, Row, Table};

    for (chunk_sz, entries) in analyze_dict {
        let mut table = Table::new();
        table.add_row(Row::new(vec![
            Cell::new("Chunk Size(KB)"),
            Cell::new("Files"),
            Cell::new("Duplicate(KB)"),
        ]));

        let mut total_sz = 0u64;
        for (files, dup_size) in entries {
            table.add_row(Row::new(vec![
                Cell::new(&chunk_sz.to_string()),
                Cell::new(files),
                Cell::new(&dup_size.to_string()),
            ]));
            total_sz += dup_size;
        }

        table.printstd();
        println!(
            "dduper:{}KB of duplicate data found with chunk size:{}KB\n\n",
            total_sz, chunk_sz
        );
    }
}
