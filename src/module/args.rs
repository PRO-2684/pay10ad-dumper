use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(next_line_help = true)]
pub struct Args {
    pub payload_path: PathBuf,

    /// Output directory for extracted partitions
    #[arg(long, short, default_value = "output")]
    pub out: PathBuf,

    #[cfg(feature = "differential_ota")]
    /// Enable differential OTA mode (requires --old)
    #[arg(long)]
    pub diff: bool,

    #[cfg(feature = "differential_ota")]
    /// Path to the directory containing old partition images (required for --diff)
    #[arg(long, default_value = "old")]
    pub old: PathBuf,

    /// Comma-separated list of partition names to extract
    #[arg(long, short, default_value = "", hide_default_value = true)]
    pub images: String,

    /// Number of threads to use for parallel processing
    #[arg(long)]
    pub threads: Option<usize>,

    #[cfg(feature = "differential_ota")]
    /// List available partitions in the payload
    #[arg(long, conflicts_with_all = &["diff", "old", "images", "threads"])]
    pub list: bool,

    #[cfg(not(feature = "differential_ota"))]
    /// List available partitions in the payload
    #[arg(long, short, conflicts_with_all = &["images", "threads"])]
    pub list: bool,

    #[cfg(feature = "differential_ota")]
    /// Save complete metadata as JSON (use --out - to write to stdout)
    #[arg(long, conflicts_with_all = &["diff", "old", "images"], hide = cfg!(not(feature = "metadata")))]
    pub metadata: bool,

    #[cfg(not(feature = "differential_ota"))]
    /// Save complete metadata as JSON (use --out - to write to stdout)
    #[arg(long, conflicts_with_all = &["images"], hide = cfg!(not(feature = "metadata")))]
    pub metadata: bool,

    /// Disable parallel extraction
    #[arg(long)]
    pub no_parallel: bool,

    /// Skip hash verification
    #[arg(long)]
    pub no_verify: bool,
}
