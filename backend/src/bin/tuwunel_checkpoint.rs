use std::env;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use rocksdb::checkpoint::Checkpoint;
use rocksdb::{Options, DB};

fn main() -> Result<()> {
    let src = env::args()
        .nth(1)
        .context("usage: tuwunel_checkpoint <source-db-dir> <checkpoint-dir>")?;
    let dest = env::args()
        .nth(2)
        .context("usage: tuwunel_checkpoint <source-db-dir> <checkpoint-dir>")?;

    if !Path::new(&src).exists() {
        bail!("source db dir does not exist: {src}");
    }
    if Path::new(&dest).exists() {
        fs::remove_dir_all(&dest)
            .with_context(|| format!("failed to remove existing checkpoint dir {dest}"))?;
    }

    let mut opts = Options::default();
    opts.create_if_missing(false);

    // List all column families first - Tuwunel uses 100+ CFs.
    // Opening without specifying all CFs only captures the default CF,
    // resulting in a checkpoint with metadata but no actual data (SST files).
    let cf_names = DB::list_cf(&opts, &src)
        .with_context(|| format!("failed to list column families at {src}"))?;
    eprintln!("Found {} column families", cf_names.len());

    let db = DB::open_cf_for_read_only(&opts, &src, &cf_names, false).with_context(|| {
        format!(
            "failed to open RocksDB at {src} with {} CFs",
            cf_names.len()
        )
    })?;

    let checkpoint = Checkpoint::new(&db).context("failed to create RocksDB checkpoint object")?;
    checkpoint
        .create_checkpoint(&dest)
        .with_context(|| format!("failed to create checkpoint at {dest}"))?;

    // Report what was checkpointed
    let file_count = fs::read_dir(&dest)
        .map(|entries| entries.count())
        .unwrap_or(0);
    let total_size: u64 = walkdir(&dest);
    eprintln!("Checkpoint created: {file_count} top-level entries, {total_size} bytes total");

    println!("{dest}");
    Ok(())
}

fn walkdir(path: &str) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = entry.metadata();
            if let Ok(m) = meta {
                if m.is_file() {
                    total += m.len();
                } else if m.is_dir() {
                    total += walkdir(&entry.path().to_string_lossy());
                }
            }
        }
    }
    total
}
