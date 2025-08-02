use std::path::PathBuf;

use clap::Parser;

#[allow(clippy::struct_excessive_bools, reason = "Clap")]
#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(next_line_help = true)]
pub struct Args {
    /// Path or URL to your payload
    pub payload_path: PathBuf,

    /// Output directory for extracted partitions
    #[arg(long, short, default_value = "output")]
    pub out: PathBuf,

    /// Enable differential OTA mode (requires --old)
    #[arg(long)]
    pub diff: bool,

    /// Path to the directory containing old partition images (required for --diff)
    #[arg(long, default_value = "old")]
    pub old: PathBuf,

    /// List of partition names to extract
    #[arg(long, short)]
    pub partitions: Vec<String>,

    /// Number of threads to use for parallel processing
    #[arg(long)]
    pub threads: Option<usize>,

    /// List available partitions in the payload
    #[arg(long, conflicts_with_all = &["diff", "old", "partitions", "threads"])]
    pub list: bool,

    /// Save complete metadata as JSON (use --out - to write to stdout)
    #[arg(long, conflicts_with_all = &["diff", "old", "partitions"])]
    pub metadata: bool,

    /// Disable parallel extraction
    #[arg(long)]
    pub no_parallel: bool,

    /// Skip hash verification
    #[arg(long)]
    pub no_verify: bool,

    /// User-Agent to use if extracting from URL (Defaults to a representative browser UA)
    #[arg(
        long,
        short,
        default_value = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        hide_default_value = true
    )]
    pub user_agent: String,
}
