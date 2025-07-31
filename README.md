# payload-dumper-rust

Android OTA payload dumper written in Rust.

### What is Payload?

Android payload is a file that contains ROM partitions like boot, system, vendor and others. Payload Dumper extracts these partitions from the `payload.bin` file.

## Features

- Extracts all or individual partitions directly from payload.bin or ROM ZIP file.
- Supports extracting individual partitions from URLs without downloading the full ROM ZIP.
- All decompression processes run in parallel for improved performance. (Can be customised by the arguments `--no-parallel` or `--threads <n>`)
- Output partitions verification
- Parallel extraction
- Selective Partition extraction
- Direct extraction from URL

## How To Use

- Download binaries for your respective platform from the [releases](https://github.com/rhythmcache/payload-dumper-rust/releases)
- If you are using a rooted android device you might want to install it as a [magisk module](https://github.com/rhythmcache/payload-dumper-rust/releases/download/0.3.0/payload_dumper-android-magisk-module.zip)
- Or Run this in termux / Linux Terminal to install
  ```
  bash <(curl -sSL "https://raw.githubusercontent.com/rhythmcache/payload-dumper-rust/main/scripts/install.sh")
  ```
- To install on windows, run this in Powershell
  ```
  powershell -NoExit -ExecutionPolicy Bypass -Command "Invoke-RestMethod -Uri 'https://raw.githubusercontent.com/rhythmcache/payload-dumper-rust/main/scripts/install.ps1' | Invoke-Expression"
  ```

### Install via Cargo

If you have Rust and Cargo installed, you can install this tool with:

```bash
cargo install payload_dumper
```
<!--
Note - Installation VIA Cargo might fail if you dont have `protobuf-compiler`, `libzip`, `zlib` and `liblzma` installed on your system
-->

## *Performance Metrics**

- Here are the performance metrics for **Payload Dumper Rust** running on a **Poco X4 Pro (SD695, 8GB RAM)** in Termux. The test file used is [comet-ota-ad1a.240530.030-98066022.zip](https://dl.google.com/dl/android/aosp/comet-ota-ad1a.240530.030-98066022.zip) (2.53GB).

| **Extraction Method**       | **Time Taken**       | **Notes**                          |
|-----------------------------|----------------------|------------------------------------|
| **Direct Payload Extraction** | **2 minutes 26 seconds** | Extracting directly from `payload.bin`. |
| **ZIP File Extraction**      | **2 minutes 30 seconds** | Extracting directly from the ZIP file. |
| **Remote URL Extraction**    | **Slower**           | Depends on network speed.          |

---

### Screenshots

- **Direct Payload Extraction**:
  ![Direct Payload Extraction](https://raw.githubusercontent.com/rhythmcache/payload-dumper-rust/main/photos/Screenshot_20250304-175923_Termux.png)

- **ZIP File Extraction**:
  ![ZIP File Extraction](https://raw.githubusercontent.com/rhythmcache/payload-dumper-rust/main/photos/Screenshot_20250304-175502_Termux.png)

- **Remote URL Extraction**:
  ![Remote URL Extraction](https://raw.githubusercontent.com/rhythmcache/payload-dumper-rust/main/photos/Screenshot_20250304-180030_Termux.png)

---


### Usage

#### Basic Usage

To extract partitions from a payload file, run the following command:

```bash
payload_dumper <path/to/payload.bin> --out output_directory
```
#### Direct ZIP Processing

It can directly process payloads from ZIP files without requiring manual extraction. Simply provide the path to the ZIP file:

```bash
./payload_dumper <path/to/ota.zip> --out <output_directory>
```

#### Remote Payloads

It can also handle payloads/zips directly using url. Simply provide the URL as path. The speed will be affected by your network quality.

```bash
./payload_dumper https://example.com/payload.bin
```

#### Individual Partitions Extraction

- To extract individual partitions from payloads/URL/zips , use `-p`/`--partitions` and specify the names of partitions you want to extract one by one. e.g. To extract `boot` and `init_boot` from `url/zip/payload` , simply run:

  ```bash
  payload_dumper --p boot --p init_boot <https://example.com/zip>
  ```

#### CLI Reference

```
Usage: payload_dumper [OPTIONS] <PAYLOAD_PATH>

Arguments:
  <PAYLOAD_PATH>
      Path to the payload file.
  --out, -o <OUT>
      Output directory for extracted partitions. [default: output]
  --diff
      Enable differential OTA mode (requires --old).
  --old <OLD>
      Path to the directory containing old partition images (required for --diff). [default: old]
  --partitions, -i <PARTITIONS>
      List of partition names to extract (default: all partitions)
  --threads <THREADS>
      Number of threads to use for parallel processing.
  --list, -l
      List available partitions
  --metadata
      Save complete metadata as json (use -o - to write to stdout)
  --no-verify
      Skip hash verification
  --no-parallel
      Disable parallel extraction
```

### Credits
- This tool is inspired from [vm03/payload_dumper](https://github.com/vm03/payload_dumper)
