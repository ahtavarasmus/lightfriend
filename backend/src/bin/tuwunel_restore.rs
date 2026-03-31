use std::env;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use rocksdb::backup::{BackupEngine, BackupEngineOptions, RestoreOptions};
use rocksdb::Env;

fn main() -> Result<()> {
    let backup_dir = env::args()
        .nth(1)
        .context("usage: tuwunel_restore <backup-engine-dir> <restore-dir>")?;
    let restore_dir = env::args()
        .nth(2)
        .context("usage: tuwunel_restore <backup-engine-dir> <restore-dir>")?;

    if !Path::new(&backup_dir).exists() {
        bail!("backup dir does not exist: {backup_dir}");
    }

    // Verify BackupEngine structure
    let shared = Path::new(&backup_dir).join("shared_checksum");
    let private = Path::new(&backup_dir).join("private");
    if !shared.exists() || !private.exists() {
        bail!(
            "not a valid BackupEngine directory (missing shared_checksum/ or private/): {backup_dir}"
        );
    }

    // Clean restore target
    if Path::new(&restore_dir).exists() {
        fs::remove_dir_all(&restore_dir)
            .with_context(|| format!("failed to remove existing restore dir: {restore_dir}"))?;
    }
    fs::create_dir_all(&restore_dir)
        .with_context(|| format!("failed to create restore dir: {restore_dir}"))?;

    // Open BackupEngine and list available backups
    let opts = BackupEngineOptions::new(&backup_dir)
        .with_context(|| format!("failed to create backup engine options for {backup_dir}"))?;
    let env = Env::new().context("failed to create rocksdb env")?;
    let mut engine = BackupEngine::open(&opts, &env).context("failed to open backup engine")?;

    let backups = engine.get_backup_info();
    if backups.is_empty() {
        bail!("no backups found in {backup_dir}");
    }
    eprintln!("Found {} backup(s):", backups.len());
    for info in &backups {
        eprintln!(
            "  backup #{}: {} bytes, timestamp {}",
            info.backup_id, info.size, info.timestamp
        );
    }

    // Verify latest backup before restoring
    let latest = backups.last().unwrap();
    eprintln!("Verifying backup #{}...", latest.backup_id);
    engine
        .verify_backup(latest.backup_id)
        .with_context(|| format!("backup #{} failed verification", latest.backup_id))?;
    eprintln!("Verification passed.");

    // Restore
    eprintln!("Restoring to {restore_dir}...");
    let restore_opts = RestoreOptions::default();
    engine
        .restore_from_latest_backup(&restore_dir, &restore_dir, &restore_opts)
        .context("restore_from_latest_backup failed")?;

    // Report results
    let file_count = count_files(Path::new(&restore_dir));
    let total_size = dir_size(Path::new(&restore_dir));
    eprintln!("Restore OK: {file_count} files, {total_size} bytes");

    // Sanity checks
    let current = Path::new(&restore_dir).join("CURRENT");
    if !current.exists() {
        bail!("restored DB missing CURRENT file - restore may be corrupt");
    }

    let sst_count = count_files_with_ext(Path::new(&restore_dir), "sst");
    eprintln!("  SST files: {sst_count}");
    if sst_count == 0 {
        bail!("restored DB has 0 SST files - restore is empty");
    }

    println!("{restore_dir}");
    Ok(())
}

fn count_files(path: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    count += 1;
                } else if m.is_dir() {
                    count += count_files(&entry.path());
                }
            }
        }
    }
    count
}

fn count_files_with_ext(path: &Path, ext: &str) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().is_some_and(|e| e == ext) {
                count += 1;
            } else if p.is_dir() {
                count += count_files_with_ext(&p, ext);
            }
        }
    }
    count
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    total += m.len();
                } else if m.is_dir() {
                    total += dir_size(&entry.path());
                }
            }
        }
    }
    total
}
