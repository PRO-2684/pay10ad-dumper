#![warn(clippy::all, clippy::nursery, clippy::pedantic, clippy::cargo)]
#![allow(clippy::multiple_crate_versions, reason = "Dependency")]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "TBD"
)]
#![allow(clippy::too_many_lines, clippy::cognitive_complexity, reason = "TBD")]

use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow, bail};
use byteorder::{BigEndian, ReadBytesExt};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use pay10ad_dumper::{
    ReadSeek,
    args::Args,
    http::HttpReader,
    metadata::save_metadata,
    payload_dumper::{create_payload_reader, dump_partition},
    proto::{DeltaArchiveManifest, PartitionUpdate},
    utils::{format_elapsed_time, format_size, is_differential_ota, list_partitions},
    verify::verify_partitions_hash,
    zip::{local_zip::ZipPayloadReader, remote_zip::RemoteZipReader},
};
use prost::Message;
use rayon::prelude::*;

static FILE_SIZE_INFO_SHOWN: AtomicBool = AtomicBool::new(false);

fn main() -> Result<()> {
    let args = Args::parse();
    let thread_count = if args.no_parallel {
        1
    } else if let Some(threads) = args.threads {
        threads
    } else {
        num_cpus::get()
    };

    rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build_global()?;

    let start_time = Instant::now();

    let multi_progress = MultiProgress::new();
    let main_pb = multi_progress.add(ProgressBar::new_spinner());
    main_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    main_pb.enable_steady_tick(Duration::from_millis(100));
    let payload_path_str = args.payload_path.to_string_lossy().to_string();

    // Check if it's a URL - only available with remote_ota feature
    let is_url =
        payload_path_str.starts_with("http://") || payload_path_str.starts_with("https://");

    // Check if it's a local ZIP file
    let is_local_zip =
        !is_url && args.payload_path.extension().and_then(|e| e.to_str()) == Some("zip");

    main_pb.set_message("Opening file...");

    if !is_url {
        if let Ok(metadata) = fs::metadata(&args.payload_path) {
            if metadata.len() > 1024 * 1024 {
                println!(
                    "Processing file: {}, size: {}",
                    payload_path_str,
                    format_size(metadata.len())
                );
            }
        }
    }

    let mut payload_reader: Box<dyn ReadSeek> = if is_url {
        use std::path::Path;

        main_pb.set_message("Initializing remote connection...");
        let url = payload_path_str.clone();
        let is_zip = Path::new(&url)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));

        let content_type = if is_zip {
            None
        } else {
            let http_reader = HttpReader::new_silent(url.clone(), &args.user_agent);
            http_reader.map_or(None, |reader| {
                let file_size = reader.content_length;
                main_pb.set_message("Connection established");
                if file_size > 1024 * 1024 && !FILE_SIZE_INFO_SHOWN.swap(true, Ordering::SeqCst) {
                    println!("- Remote file size: {}", format_size(file_size));
                }
                reader.content_type
            })
        };

        if is_zip || content_type.as_deref() == Some("application/zip") {
            let reader = RemoteZipReader::new_for_parallel(url, &args.user_agent)?;
            let file_size = reader.http_reader.content_length;
            main_pb.set_message("Connection established");
            if file_size > 1024 * 1024 && !FILE_SIZE_INFO_SHOWN.swap(true, Ordering::SeqCst) {
                println!("- Remote ZIP size: {}", format_size(file_size));
            }
            Box::new(reader) as Box<dyn ReadSeek>
        } else {
            let reader = HttpReader::new(url, &args.user_agent)?;
            let file_size = reader.content_length;
            main_pb.set_message("Connection established");
            if file_size > 1024 * 1024 && !FILE_SIZE_INFO_SHOWN.swap(true, Ordering::SeqCst) {
                println!("- Remote file size: {}", format_size(file_size));
            }
            Box::new(reader) as Box<dyn ReadSeek>
        }
    } else if is_local_zip {
        ZipPayloadReader::<std::fs::File>::from_file(&args.payload_path)
            .map_err(|e| anyhow::anyhow!("Failed to open ZIP file: {e}"))
            .map(|reader| Box::new(reader) as Box<dyn ReadSeek>)?
    } else {
        Box::new(File::open(&args.payload_path)?) as Box<dyn ReadSeek>
    };

    if args.out.to_string_lossy() != "-" {
        fs::create_dir_all(&args.out)?;
    }

    let mut magic = [0u8; 4];
    payload_reader.read_exact(&mut magic)?;
    if magic != *b"CrAU" {
        bail!("Invalid payload file: magic 'CrAU' not found");
    }
    let file_format_version = payload_reader.read_u64::<BigEndian>()?;
    if file_format_version != 2 {
        bail!("Unsupported payload version: {file_format_version}");
    }
    let manifest_size = payload_reader.read_u64::<BigEndian>()?;
    let metadata_signature_size = payload_reader.read_u32::<BigEndian>()?;
    main_pb.set_message("Reading manifest...");
    let mut manifest = vec![0u8; manifest_size as usize];
    payload_reader.read_exact(&mut manifest)?;
    let mut metadata_signature = vec![0u8; metadata_signature_size as usize];
    payload_reader.read_exact(&mut metadata_signature)?;
    let data_offset = payload_reader.stream_position()?;
    let manifest = DeltaArchiveManifest::decode(&manifest[..])?;

    if is_differential_ota(&manifest) && !args.diff {
        bail!(
            "This appears to be a differential OTA package. Use --diff argument and provide the original partitions directory with --old <path>"
        );
    }

    if let Some(security_patch) = &manifest.security_patch_level {
        println!("- Security Patch: {security_patch}");
    }

    if args.metadata && !args.list {
        main_pb.set_message("Extracting metadata...");
        let is_stdout = args.out.to_string_lossy() == "-";

        match save_metadata(&manifest, &args.out, data_offset) {
            Ok(json) => {
                if is_stdout {
                    println!("{json}");
                } else {
                    println!(
                        "✓ Metadata saved to: {}/payload_metadata.json",
                        args.out.display()
                    );
                }
                multi_progress.clear()?;
                return Ok(());
            }
            Err(e) => {
                main_pb.finish_with_message("Failed to save metadata");
                return Err(e);
            }
        }
    }

    if args.list {
        main_pb.finish_and_clear();
        multi_progress.clear()?;

        if args.metadata {
            let is_stdout = args.out.to_string_lossy() == "-";

            match save_metadata(&manifest, &args.out, data_offset) {
                Ok(json) => {
                    if is_stdout {
                        println!("{json}");
                        return Ok(());
                    }
                    println!(
                        "✓ Metadata saved to: {}/payload_metadata.json",
                        args.out.display()
                    );
                }
                Err(e) => {
                    eprintln!("Failed to save metadata: {e}");
                }
            }
        }

        println!();
        payload_reader.seek(SeekFrom::Start(0))?;
        return list_partitions(&mut payload_reader);
    }

    let block_size = manifest.block_size.unwrap_or(4096);
    let partitions_to_extract: Vec<_> = if args.partitions.is_empty() {
        manifest.partitions.iter().collect()
    } else {
        let partitions: HashSet<_> = args.partitions.iter().collect();
        manifest
            .partitions
            .iter()
            .filter(|p| partitions.contains(&p.partition_name))
            .collect()
    };
    if partitions_to_extract.is_empty() {
        main_pb.finish_with_message("No partitions to extract");
        multi_progress.clear()?;
        return Ok(());
    }
    main_pb.set_message(format!(
        "Found {} partitions to extract",
        partitions_to_extract.len()
    ));

    let use_parallel = (args.payload_path.extension().and_then(|e| e.to_str()) == Some("bin")
        || is_local_zip
        || is_url)
        && !args.no_parallel;
    main_pb.set_message(if use_parallel {
        "Extracting Partitions..."
    } else {
        "Processing partitions..."
    });
    let multi_progress = Arc::new(multi_progress);
    let args = Arc::new(args);

    let mut failed_partitions = Vec::new();

    if use_parallel {
        let payload_path = Arc::new(args.payload_path.to_str().unwrap_or_default().to_string());
        let payload_url = Arc::new(if is_url {
            payload_path_str
        } else {
            String::new()
        });

        let max_retries = 3;
        let num_cpus = num_cpus::get();
        let chunk_size = std::cmp::max(1, partitions_to_extract.len() / num_cpus);

        let active_readers = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_concurrent_readers = num_cpus;

        let results: Vec<_> = partitions_to_extract
            .par_chunks(chunk_size)
            .flat_map(|chunk| {
                chunk.par_iter().map(|partition| {
                    let active_readers = Arc::clone(&active_readers);

                    (0..max_retries)
                        .find_map(|attempt| {
                            if attempt > 0 {
                                let delay = 100 * (1 << attempt.min(4));
                                std::thread::sleep(Duration::from_millis(delay));
                            }

                            let needs_reader_limit = !is_url && is_local_zip;

                            if needs_reader_limit {
                                let current =
                                    active_readers.load(std::sync::atomic::Ordering::SeqCst);
                                if current >= max_concurrent_readers {
                                    while active_readers.load(std::sync::atomic::Ordering::SeqCst)
                                        >= max_concurrent_readers
                                    {
                                        std::thread::sleep(Duration::from_millis(10));
                                    }
                                }

                                active_readers.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            }

                            let reader_result = if is_url {
                                RemoteZipReader::new_for_parallel(
                                    (*payload_url).clone(),
                                    &args.user_agent,
                                )
                                .map(|reader| Box::new(reader) as Box<dyn ReadSeek>)
                                // Fix for the error type mismatch around line 384-398
                            } else if is_local_zip {
                                let result =
                                    ZipPayloadReader::new_for_parallel((*payload_path).clone())
                                        .map(|reader| Box::new(reader) as Box<dyn ReadSeek>)
                                        .map_err(|e| {
                                            anyhow::anyhow!("Failed to create ZIP reader: {e}")
                                        }); // Convert io::Error to anyhow::Error
                                active_readers.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                                result
                            } else {
                                create_payload_reader(&args.payload_path).map_err(|e| {
                                    anyhow::anyhow!("Failed to create payload reader: {e}")
                                })
                            };

                            let mut reader = match reader_result {
                                Ok(reader) => reader,
                                Err(e) => {
                                    return if attempt == max_retries - 1 {
                                        Some(Err((partition.partition_name.clone(), e)))
                                    } else {
                                        None // Try again
                                    };
                                }
                            };

                            match dump_partition(
                                partition,
                                data_offset,
                                u64::from(block_size),
                                &args,
                                &mut reader,
                                Some(&multi_progress),
                            ) {
                                Ok(()) => Some(Ok(())),
                                Err(e) => {
                                    if attempt == max_retries - 1 {
                                        Some(Err((partition.partition_name.clone(), e)))
                                    } else {
                                        None // Try again
                                    }
                                }
                            }
                        })
                        .unwrap_or_else(|| {
                            Err((
                                partition.partition_name.clone(),
                                anyhow!("All retry attempts failed"),
                            ))
                        })
                })
            })
            .collect();
        for result in results {
            if let Err((partition_name, error)) = result {
                eprintln!("Failed to process partition {partition_name}: {error}");
                failed_partitions.push(partition_name);
            }
        }
        if !failed_partitions.is_empty() {
            main_pb.set_message(format!(
                "Retrying {} failed partitions sequentially...",
                failed_partitions.len()
            ));

            let mut reader: Box<dyn ReadSeek> = if is_url {
                Box::new(RemoteZipReader::new_for_parallel(
                    payload_url.to_string(),
                    &args.user_agent,
                )?) as Box<dyn ReadSeek>
            } else {
                payload_reader
            };

            let mut remaining_failed_partitions = Vec::new();
            for partition in partitions_to_extract
                .iter()
                .filter(|p| failed_partitions.contains(&p.partition_name))
            {
                if let Err(e) = dump_partition(
                    partition,
                    data_offset,
                    u64::from(block_size),
                    &args,
                    &mut reader,
                    Some(&multi_progress),
                ) {
                    eprintln!(
                        "Failed to process partition {} in sequential mode: {}",
                        partition.partition_name, e
                    );
                    remaining_failed_partitions.push(partition.partition_name.clone());
                }
            }
            failed_partitions = remaining_failed_partitions;
        }
    } else {
        for partition in &partitions_to_extract {
            if let Err(e) = dump_partition(
                partition,
                data_offset,
                u64::from(block_size),
                &args,
                &mut payload_reader,
                Some(&multi_progress),
            ) {
                eprintln!(
                    "Failed to process partition {}: {}",
                    partition.partition_name, e
                );
                failed_partitions.push(partition.partition_name.clone());
            }
        }
    }

    if args.no_verify {
        main_pb.set_message("Hash verification skipped (--no-verify flag)");
    } else {
        main_pb.set_message("Verifying partition hashes...");

        let partitions_to_verify: Vec<&PartitionUpdate> = partitions_to_extract
            .iter()
            .filter(|p| !failed_partitions.contains(&p.partition_name))
            .copied()
            .collect();

        let failed_verifications =
            verify_partitions_hash(&partitions_to_verify, &args, &multi_progress);
        if !failed_verifications.is_empty() {
            eprintln!(
                "Hash verification failed for {} partitions.",
                failed_verifications.len()
            );
        }
    }

    let elapsed_time = format_elapsed_time(start_time.elapsed());

    if failed_partitions.is_empty() {
        main_pb.finish_with_message(format!(
            "All partitions extracted successfully! (in {elapsed_time})"
        ));
        println!(
            "\nExtraction completed successfully in {}. Output directory: {}",
            elapsed_time,
            args.out.display()
        );
    } else {
        main_pb.finish_with_message(format!(
            "Completed with {} failed partitions. (in {})",
            failed_partitions.len(),
            elapsed_time
        ));
        println!(
            "\nExtraction completed with {} failed partitions in {}. Output directory: {}",
            failed_partitions.len(),
            elapsed_time,
            args.out.display()
        );
    }

    Ok(())
}
