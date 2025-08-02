#![warn(clippy::all, clippy::nursery, clippy::pedantic, clippy::cargo)]
#![allow(clippy::multiple_crate_versions, reason = "Dependency")]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "TBD"
)]
#![allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    reason = "TBD"
)]

#[cfg(feature = "cli")]
pub mod args;
pub mod http;
pub mod metadata;
pub mod patch;
pub mod payload_dumper;
pub mod proto;
pub mod structs;
pub mod utils;
pub mod verify;
pub mod zip;

use std::io::{Read, Seek};

pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}
