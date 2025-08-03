use std::path::PathBuf;

use argh::FromArgs;

#[allow(clippy::struct_excessive_bools, reason = "CLI")]
#[derive(FromArgs)]
/// Feature-rich Android OTA payload dumper written in Rust
pub struct Args {
    /// path or URL to your payload
    #[argh(positional)]
    pub payload_path: PathBuf,

    /// output directory for extracted partitions
    #[argh(option, short = 'o', default = "\"output\".into()")]
    pub out: PathBuf,

    /// enable differential OTA mode (requires --old)
    #[argh(switch)]
    pub diff: bool,

    /// path to the directory containing old partition images (required for --diff)
    #[argh(option, default = "\"old\".into()")]
    pub old: PathBuf,

    /// list of partition names to extract
    #[argh(option, short = 'p')]
    pub partitions: Vec<String>,

    /// number of threads to use for parallel processing
    #[argh(option)]
    pub threads: Option<usize>,

    /// list available partitions in the payload
    // TODO: Conflict with ["diff", "old", "partitions", "threads"]
    #[argh(switch, short = 'l')]
    pub list: bool,

    /// save complete metadata as JSON (use --out - to write to stdout)
    // TODO: Conflict with ["diff", "old", "partitions"]
    #[argh(switch)]
    pub metadata: bool,

    /// disable parallel extraction
    #[argh(switch)]
    pub no_parallel: bool,

    /// skip hash verification
    #[argh(switch)]
    pub no_verify: bool,

    /// the User-Agent to use if extracting from URL (Defaults to a representative browser UA)
    #[argh(
        option,
        short = 'u',
        default = "\"Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36\".into()"
    )]
    pub user_agent: String,
}
