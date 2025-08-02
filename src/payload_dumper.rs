use crate::ReadSeek;
use crate::args::Args;
use crate::patch::bspatch;
use crate::proto::{InstallOperation, PartitionUpdate, install_operation};
use crate::verify::verify_hash;
use crate::verify::verify_old_partition;
use anyhow::{Context, Result, anyhow, bail};
use bzip2::read::BzDecoder;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;
use xz4rust::{XzDecoder, XzNextBlockResult};

pub fn process_operation(
    operation_index: usize,
    op: &InstallOperation,
    data_offset: u64,
    block_size: u64,
    payload_file: &mut (impl Read + Seek),
    out_file: &mut (impl Write + Seek),
    old_file: Option<&mut dyn ReadSeek>,
) -> Result<()> {
    payload_file.seek(SeekFrom::Start(data_offset + op.data_offset.unwrap_or(0)))?;
    let mut data = vec![0u8; op.data_length.unwrap_or(0) as usize];
    payload_file.read_exact(&mut data)?;

    if let Some(expected_hash) = op.data_sha256_hash.as_deref() {
        if !verify_hash(&data, expected_hash) {
            println!("  Warning: Operation {operation_index} data hash mismatch.");
            return Ok(());
        }
    }
    match op.r#type() {
        install_operation::Type::ReplaceXz => {
            let mut decompressed = Vec::new();
            let mut decoder = XzDecoder::in_heap_with_alloc_dict_size(
                xz4rust::DICT_SIZE_MIN,
                xz4rust::DICT_SIZE_MAX,
            );

            let mut input_position = 0;
            let mut temp_buffer = [0u8; 4096];

            loop {
                match decoder.decode(&data[input_position..], &mut temp_buffer) {
                    Ok(XzNextBlockResult::NeedMoreData(input_consumed, output_produced)) => {
                        input_position += input_consumed;
                        decompressed.extend_from_slice(&temp_buffer[..output_produced]);
                    }
                    Ok(XzNextBlockResult::EndOfStream(_, output_produced)) => {
                        decompressed.extend_from_slice(&temp_buffer[..output_produced]);
                        break;
                    }
                    Err(e) => {
                        println!(
                            "  Warning: Skipping operation {operation_index} due to XZ decompression error: {e}"
                        );
                        return Ok(());
                    }
                }
            }

            out_file.seek(SeekFrom::Start(
                op.dst_extents[0].start_block.unwrap_or(0) * block_size,
            ))?;
            out_file.write_all(&decompressed)?;
        }
        install_operation::Type::Zstd => match zstd::decode_all(Cursor::new(&data)) {
            Ok(decompressed) => {
                let mut pos = 0;
                for ext in &op.dst_extents {
                    let ext_size = (ext.num_blocks.unwrap_or(0) * block_size) as usize;
                    let end_pos = pos + ext_size;

                    if end_pos <= decompressed.len() {
                        out_file
                            .seek(SeekFrom::Start(ext.start_block.unwrap_or(0) * block_size))?;
                        out_file.write_all(&decompressed[pos..end_pos])?;
                        pos = end_pos;
                    } else {
                        println!(
                            "  Warning: Skipping extent in operation {operation_index} due to insufficient decompressed data."
                        );
                        break;
                    }
                }
            }
            Err(e) => {
                println!(
                    "  Warning: Skipping operation {operation_index} due to unknown Zstd format: {e}"
                );
                return Ok(());
            }
        },
        install_operation::Type::ReplaceBz => {
            let mut decoder = BzDecoder::new(Cursor::new(&data));
            let mut decompressed = Vec::new();
            match decoder.read_to_end(&mut decompressed) {
                Ok(_) => {
                    out_file.seek(SeekFrom::Start(
                        op.dst_extents[0].start_block.unwrap_or(0) * block_size,
                    ))?;
                    out_file.write_all(&decompressed)?;
                }
                Err(e) => {
                    println!(
                        " Warning: Skipping operation {operation_index} due to unknown BZ2 format.  : {e}"
                    );
                    return Ok(());
                }
            }
        }
        install_operation::Type::Replace => {
            out_file.seek(SeekFrom::Start(
                op.dst_extents[0].start_block.unwrap_or(0) * block_size,
            ))?;
            out_file.write_all(&data)?;
        }
        install_operation::Type::SourceCopy => {
            let old_file = old_file
                .ok_or_else(|| anyhow!("SOURCE_COPY supported only for differential OTA"))?;
            out_file.seek(SeekFrom::Start(
                op.dst_extents[0].start_block.unwrap_or(0) * block_size,
            ))?;
            for ext in &op.src_extents {
                old_file.seek(SeekFrom::Start(ext.start_block.unwrap_or(0) * block_size))?;
                let mut buffer = vec![0u8; (ext.num_blocks.unwrap_or(0) * block_size) as usize];
                old_file.read_exact(&mut buffer)?;
                out_file.write_all(&buffer)?;
            }
        }
        install_operation::Type::SourceBsdiff | install_operation::Type::BrotliBsdiff => {
            let old_file =
                old_file.ok_or_else(|| anyhow!("BSDIFF supported only for differential OTA"))?;

            let mut old_data = Vec::new();
            for ext in &op.src_extents {
                old_file.seek(SeekFrom::Start(ext.start_block.unwrap_or(0) * block_size))?;
                let mut buffer = vec![0u8; (ext.num_blocks.unwrap_or(0) * block_size) as usize];
                old_file.read_exact(&mut buffer)?;
                old_data.extend_from_slice(&buffer);
            }
            let new_data = match bspatch(&old_data, &data) {
                Ok(new_data) => new_data,
                Err(e) => {
                    println!(
                        "  Warning: Skipping operation {operation_index} due to failed BSDIFF patch.  : {e}"
                    );
                    return Ok(());
                }
            };
            let mut pos = 0;
            for ext in &op.dst_extents {
                let ext_size = (ext.num_blocks.unwrap_or(0) * block_size) as usize;
                let end_pos = pos + ext_size;
                if end_pos <= new_data.len() {
                    out_file.seek(SeekFrom::Start(ext.start_block.unwrap_or(0) * block_size))?;
                    out_file.write_all(&new_data[pos..end_pos])?;
                    pos = end_pos;
                } else {
                    println!(
                        "  Warning: Skipping operation {operation_index} due to insufficient patched data.  ."
                    );
                    return Ok(());
                }
            }
        }
        install_operation::Type::Zero => {
            let zeros = vec![0u8; block_size as usize];
            for ext in &op.dst_extents {
                out_file.seek(SeekFrom::Start(ext.start_block.unwrap_or(0) * block_size))?;
                for _ in 0..ext.num_blocks.unwrap_or(0) {
                    out_file.write_all(&zeros)?;
                }
            }
        }
        _ => {
            println!(
                "  Warning: Skipping operation {operation_index} due to unknown compression method"
            );
            return Ok(());
        }
    }
    Ok(())
}

pub fn dump_partition(
    partition: &PartitionUpdate,
    data_offset: u64,
    block_size: u64,
    args: &Args,
    payload_file: &mut (impl Read + Seek),
    multi_progress: Option<&MultiProgress>,
) -> Result<()> {
    let partition_name = &partition.partition_name;
    let total_ops = partition.operations.len() as u64;
    let progress_bar = multi_progress.map_or_else(|| None, |mp| {
        let pb = mp.add(ProgressBar::new(100));
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/white}] {percent}% - {msg}")
            .unwrap()
            .progress_chars("▰▱"));
        pb.enable_steady_tick(Duration::from_millis(500));
        pb.set_message(format!("Processing {partition_name} ({total_ops} ops)"));
        Some(pb)
    });
    let out_dir = &args.out;
    if args.out.to_string_lossy() != "-" {
        fs::create_dir_all(out_dir)?;
    }
    let out_path = out_dir.join(format!("{partition_name}.img"));
    let mut out_file = File::create(&out_path)?;

    if let Some(info) = &partition.new_partition_info {
        if info.size.unwrap_or(0) > 0 {
            #[cfg(target_family = "unix")]
            {
                if let Some(size) = info.size {
                    out_file.set_len(size)?;
                } else {
                    bail!("Partition size is missing");
                }
            }
        }
    }

    let mut old_file = if args.diff {
        let old_path = args.old.join(format!("{partition_name}.img"));
        let mut file = File::open(&old_path)
            .with_context(|| format!("Failed to open original image: {}", old_path.display()))?;

        // Verify old partition hash if available
        if let Some(old_partition_info) = &partition.old_partition_info {
            if let Err(e) = verify_old_partition(&mut file, old_partition_info) {
                bail!("Old partition verification failed for {partition_name}: {e}");
            }
        }

        Some(file)
    } else {
        None
    };

    for (i, op) in partition.operations.iter().enumerate() {
        process_operation(
            i,
            op,
            data_offset,
            block_size,
            payload_file,
            &mut out_file,
            old_file.as_mut().map(|f| f as &mut dyn ReadSeek),
        )?;

        if let Some(pb) = &progress_bar {
            let percentage = ((i + 1) as f64 / total_ops as f64 * 100.0) as u64;
            pb.set_position(percentage);
        }
    }
    if let Some(pb) = progress_bar {
        pb.finish_with_message(format!("✓ Completed {partition_name} ({total_ops} ops)"));
    }
    drop(out_file);
    let mut out_file = File::open(&out_path)
        .with_context(|| format!("Failed to reopen {partition_name} for hash verification"))?;
    if let Some(info) = &partition.new_partition_info {
        if info.hash.as_ref().is_none_or(std::vec::Vec::is_empty) {
            let hash_pb = multi_progress.map_or_else(
                || None,
                |mp| {
                    let pb = mp.add(ProgressBar::new_spinner());
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .template("{spinner:.green} {msg}")
                            .unwrap(),
                    );
                    pb.enable_steady_tick(Duration::from_millis(100));
                    pb.set_message(format!("Verifying hash for {partition_name}"));
                    Some(pb)
                },
            );
            out_file.seek(SeekFrom::Start(0))?;
            let mut hasher = Sha256::new();
            io::copy(&mut out_file, &mut hasher)?;
            let hash = hasher.finalize();
            if let Some(pb) = hash_pb {
                if hash.as_slice() == info.hash.as_deref().unwrap_or(&[]) {
                    pb.finish_with_message(format!("✓ Hash verified for {partition_name}"));
                } else {
                    pb.finish_with_message(format!("✕ Hash mismatch for {partition_name}"));
                }
            }
        }
    }
    Ok(())
}

pub fn create_payload_reader(path: &PathBuf) -> Result<Box<dyn ReadSeek>> {
    let file = File::open(path)?;

    let file_size = file.metadata()?.len();

    if file_size > 10 * 1024 * 1024 {
        unsafe { memmap2::Mmap::map(&file) }.map_or_else(
            |_| Ok(Box::new(file) as Box<dyn ReadSeek>),
            |mmap| Ok(Box::new(MmapReader { mmap, position: 0 }) as Box<dyn ReadSeek>),
        )
    } else {
        Ok(Box::new(file) as Box<dyn ReadSeek>)
    }
}

struct MmapReader {
    mmap: memmap2::Mmap,
    position: u64,
}

impl Read for MmapReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let start = self.position as usize;
        if start >= self.mmap.len() {
            return Ok(0); // EOF
        }

        let end = std::cmp::min(start + buf.len(), self.mmap.len());
        let bytes_to_read = end - start;

        buf[..bytes_to_read].copy_from_slice(&self.mmap[start..end]);
        self.position += bytes_to_read as u64;

        Ok(bytes_to_read)
    }
}

impl Seek for MmapReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => {
                if offset >= 0 {
                    self.position.saturating_add(offset as u64)
                } else {
                    self.position.saturating_sub(offset.unsigned_abs())
                }
            }
            SeekFrom::End(offset) => {
                let file_size = self.mmap.len() as u64;
                if offset >= 0 {
                    file_size.saturating_add(offset as u64)
                } else {
                    file_size.saturating_sub(offset.unsigned_abs())
                }
            }
        };

        if new_pos > self.mmap.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Attempted to seek past end of file",
            ));
        }

        self.position = new_pos;
        Ok(self.position)
    }
}
