# Pay10ad Dumper

Feature-rich Android OTA payload dumper written in Rust.

> What is Payload?
> Android payload is a file that contains ROM partitions like boot, system, vendor and others. Payload Dumper extracts these partitions from the `payload.bin` file.

## 🪄 Features

- Extract partitions **selectively** (Specify via `-p`/`--partitions`)
- Extract from local `payload.bin` or ROM **zip** file without decompressing the whole archive
- Extract from **HTTP(S) URL** (`payload.bin` or zip) without downloading the whole file (Need server support)
- Verify output partitions
- Parallelism to maximize speed (Customizable via `--no-parallel`/`--threads`)
- Tiny: < 1M compressed on all supported platforms (Windows, MacOS, Linux)

## 📥 Installation

### Using [`binstall`](https://github.com/cargo-bins/cargo-binstall)

```shell
cargo binstall pay10ad-dumper
```

### Downloading from Releases

Navigate to the [Releases page](https://github.com/PRO-2684/pay10ad-dumper/releases) and download respective binary for your platform. Make sure to give it execute permissions.

### Compiling from Source

```shell
cargo install pay10ad-dumper
```

## 📖 Usage

### Common Usage

- Extract all partitions from `payload.bin`: `pay10ad-dumper payload.bin`
- List partitions from `ota.zip`: `pay10ad-dumper -l ota.zip`
- Extract `boot` & `init_boot` from `<URL>`: `pay10ad-dumper -p boot -p init_boot <URL>`

<details><summary>

📸 Screenshot of `pay10ad-dumper` extracting `init_boot.img` from an online OTA zip file with specified UA

</summary>

![sample-online-zip.png](images/sample-online-zip.png)

</details>

<details><summary>

📸 Screenshot of `pay10ad-dumper` listing partitions from local `payload.bin`

</summary>

![sample-local-list](images/sample-local-list.png)

</details>

<details><summary>

📸 Screenshot of `pay10ad-dumper` listing partitions from remote `ota.zip`

</summary>

![sample-remote-list](images/sample-remote-list.png)

</details>

### CLI Reference

```shell
$ pay10ad-dumper
Feature-rich Android OTA payload dumper written in Rust

Usage: pay10ad-dumper [OPTIONS] <PAYLOAD_PATH>

Arguments:
  <PAYLOAD_PATH>
          Path or URL to your payload

Options:
  -o, --out <OUT>
          Output directory for extracted partitions [default: output]
  -p, --partitions <PARTITIONS>
          List of partition names to extract
      --threads <THREADS>
          Number of threads to use for parallel processing
  -l, --list
          List available partitions in the payload
      --metadata
          Save complete metadata as JSON (use --out - to write to stdout)
      --no-parallel
          Disable parallel extraction
      --no-verify
          Skip hash verification
  -u, --user-agent <USER_AGENT>
          User-Agent to use (Only takes effect when providing URL) [default: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"]
  -h, --help
          Print help
  -V, --version
          Print version
```

## 🤔 Comparison

| Project | Lang | Zip | URL | URL + zip | Size |
| - | - | - | - | - | - |
| [`vm03/payload_dumper`](https://github.com/vm03/payload_dumper) | 🐍 Python | 🔴 | 🔴 | 🔴 | N/A |
| [`5ec1cff/payload-dumper`](https://github.com/5ec1cff/payload-dumper) | 🐍 Python | 🟢 | 🟢 | 🟡 | N/A |
| [`payload-dumper-go`](https://github.com/ssut/payload-dumper-go) | 🐹 Go | 🟢 | 🔴 | 🔴 | ≈5M |
| [`payload-dumper-rust`](https://github.com/rhythmcache/payload-dumper-rust) | 🦀 Rust | 🟢 | 🟢 | 🟢 | ≈2M |
| [`pay10ad-dumper`](https://github.com/PRO-2684/pay10ad-dumper) | 🦀 Rust | 🟢 | 🟢 | 🟢 | ≈1.5M |

Additional explanation:

- Heading
    - `Zip`: Whether it supports extracting partitions from ZIP archives without decompressing it first.
    - `URL`: Whether it supports extracting partitions from `payload.bin` URLs without downloading the whole file.
    - `URL + Zip`: Whether it supports extracting partitions from `ota.zip` URLs without downloading the whole file.
    - `Size`: Compiled executable size (decompressed).
- Values
    - `🔴`: No
    - `🟢`: Yes
    - `🟡`: Not tested

## 🎉 Credits

- Forked from [rhythmcache/payload-dumper-rust](https://github.com/rhythmcache/payload-dumper-rust) to scratch my own itch (Set User-Agent)
