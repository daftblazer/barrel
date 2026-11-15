//! WMA 9 Pro (WMA Professional) audio decoding for Barrel's movie playback.
//!
//! Written in Rust after FFmpeg's `libavcodec/wmaprodec.c` (Sascha Sommer, Benjamin Larsson
//! and the FFmpeg developers), the only public description of the format; like it, this crate
//! is under the GNU Lesser General Public License, version 2.1 or later (`COPYING.LGPLv2.1`).
//! The rest of Barrel is MIT and only calls this crate; it does not link FFmpeg.

pub mod bits;
pub mod frames;
pub mod params;

pub use frames::{FrameHeader, Packets};
pub use params::Params;

/// A decoding error: what was being read when the data did not fit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
