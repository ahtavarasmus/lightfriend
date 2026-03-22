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

    let db = DB::open_for_read_only(&opts, &src, false)
        .with_context(|| format!("failed to open RocksDB at {src}"))?;
    let checkpoint = Checkpoint::new(&db).context("failed to create RocksDB checkpoint object")?;
    checkpoint
        .create_checkpoint(&dest)
        .with_context(|| format!("failed to create checkpoint at {dest}"))?;

    println!("{dest}");
    Ok(())
}
