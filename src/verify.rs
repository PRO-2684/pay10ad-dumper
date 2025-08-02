use crate::ReadSeek;
use crate::args::Args;
use crate::proto::{PartitionInfo, PartitionUpdate};
use crate::utils::format_size;
use anyhow::bail;
use anyhow::{Context, Result};
use digest::Digest;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::Sha256;
use std::fs::File;
use std::io::Read;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::time::Duration;

#[must_use]
pub fn verify_hash(data: &[u8], expected_hash: &[u8]) -> bool {
    if expected_hash.is_empty() {
        return true;
    }
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();

    hash.as_slice() == expected_hash
}

#[must_use]
pub fn verify_partitions_hash(
    partitions: &[&PartitionUpdate],
    args: &Args,
    multi_progress: &MultiProgress,
) -> Vec<String> {
    if args.no_verify {
        return vec![];
    }

    let verification_pb = multi_progress.add(ProgressBar::new_spinner());
    verification_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    verification_pb.enable_steady_tick(Duration::from_millis(100));
    verification_pb.set_message(format!(
        "Verifying hashes for {} partitions",
        partitions.len()
    ));

    let out_dir = &args.out;
    let mut failed_verifications = Vec::new();
    let progress_bars: Vec<_> = partitions
        .iter()
        .map(|partition| {
            let pb = multi_progress.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            pb.enable_steady_tick(Duration::from_millis(100));
            pb.set_message(format!("Queuing {}", partition.partition_name));
            (partition.partition_name.clone(), pb)
        })
        .collect();

    let results: Vec<_> = partitions
        .par_iter()
        .map(|partition| {
            let partition_name = &partition.partition_name;
            let out_path = out_dir.join(format!("{partition_name}.img"));

            let expected_hash = partition
                .new_partition_info
                .as_ref()
                .and_then(|info| info.hash.as_ref());

            let pb = progress_bars
                .iter()
                .find(|(name, _)| name == partition_name)
                .map(|(_, pb)| pb.clone());

            if let Some(pb) = &pb {
                pb.set_message(format!("Verifying {partition_name}"));
            }

            let result = verify_partition_hash(partition_name, &out_path, expected_hash, pb);

            match result {
                Ok(true) => Ok(partition_name.clone()),
                Ok(false) => Err(partition_name.clone()),
                Err(e) => {
                    eprintln!("Error verifying hash for {partition_name}: {e}");
                    Err(partition_name.clone())
                }
            }
        })
        .collect();

    for result in results {
        if let Err(partition_name) = result {
            failed_verifications.push(partition_name);
        }
    }

    if failed_verifications.is_empty() {
        verification_pb.finish_with_message("All hashes verified successfully");
    } else {
        verification_pb.finish_with_message(format!(
            "Hash verification completed with {} failures",
            failed_verifications.len()
        ));
    }

    failed_verifications
}

pub fn verify_partition_hash(
    partition_name: &str,
    out_path: &PathBuf,
    expected_hash: Option<&Vec<u8>>,
    progress_bar: Option<ProgressBar>,
) -> Result<bool> {
    if let Some(expected) = expected_hash {
        if expected.is_empty() {
            if let Some(pb) = progress_bar {
                pb.finish_with_message(format!("No hash for {partition_name}"));
            }
            return Ok(true);
        }

        let file = File::open(out_path)
            .with_context(|| format!("Failed to open {partition_name} for hash verification"))?;

        let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);

        if let Some(pb) = &progress_bar {
            pb.set_message(format!(
                "Verifying {} ({})",
                partition_name,
                format_size(file_size)
            ));
        }

        let mut hasher = Sha256::new();

        if file_size > 10 * 1024 * 1024 {
            if let Ok(mmap) = unsafe { memmap2::Mmap::map(&file) } {
                hasher.update(&mmap[..]);

                let hash = hasher.finalize();
                let matches = hash.as_slice() == expected.as_slice();

                if let Some(pb) = progress_bar {
                    if matches {
                        pb.finish_with_message(format!("✓ {partition_name} verified"));
                    } else {
                        pb.finish_with_message(format!("✕ {partition_name} mismatch"));
                    }
                }

                return Ok(matches);
            }
            // Fall back
        }

        let buffer_size = if file_size < 1024 * 1024 {
            64 * 1024
        } else if file_size < 100 * 1024 * 1024 {
            1024 * 1024
        } else {
            8 * 1024 * 1024
        };

        let mut file = std::io::BufReader::new(file);
        let mut buffer = vec![0u8; buffer_size];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        let hash = hasher.finalize();
        let matches = hash.as_slice() == expected.as_slice();

        if let Some(pb) = progress_bar {
            if matches {
                pb.finish_with_message(format!("✓ {partition_name} verified"));
            } else {
                pb.finish_with_message(format!("✕ {partition_name} mismatch"));
            }
        }

        Ok(matches)
    } else {
        if let Some(pb) = progress_bar {
            pb.finish_with_message(format!("No hash for {partition_name}"));
        }
        Ok(true)
    }
}

pub fn verify_old_partition(
    old_file: &mut dyn ReadSeek,
    old_partition_info: &PartitionInfo,
) -> Result<()> {
    if let Some(expected_hash) = old_partition_info.hash.as_deref() {
        if expected_hash.is_empty() {
            return Ok(()); // No hash to verify
        }

        old_file.seek(SeekFrom::Start(0))?;
        let mut hasher = Sha256::new();

        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer
        loop {
            let bytes_read = old_file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let computed_hash = hasher.finalize();
        if computed_hash.as_slice() != expected_hash {
            bail!("Old partition hash verification failed");
        }
    }
    Ok(())
}
