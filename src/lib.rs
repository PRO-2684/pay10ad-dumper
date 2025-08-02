pub mod args;
#[cfg(feature = "remote_ota")]
pub mod http;
#[cfg(feature = "metadata")]
pub mod metadata;
#[cfg(feature = "differential_ota")]
pub mod patch;
pub mod payload_dumper;
#[cfg(feature = "metadata")]
pub mod structs;
pub mod utils;
pub mod verify;
pub mod zip;
pub mod proto;

use std::io::{Read, Seek};

pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}
