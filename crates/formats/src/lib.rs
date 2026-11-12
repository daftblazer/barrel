//! Readers for the data formats of Gust's Atelier games.
//!
//! Every reader works on bytes already in memory, checks each count and offset it reads
//! against the input, and returns [`Error`] rather than panicking on bad data.

pub mod battle_data;
pub mod bgm;
pub mod costume;
pub mod bc;
pub mod chara_div;
pub mod bc6h;
pub mod bc7;
pub mod dxbc;
pub mod ebd;
pub mod ebm;
mod ebd_tables;
pub mod elixir;
pub mod event_check;
pub mod event_check_codes;
pub mod event_process;
pub mod font;
pub mod field_look;
pub mod g1a;
pub mod gather;
pub mod gimmick;
pub mod g1h;
pub mod g1m;
pub mod g1t;
pub mod game;
pub mod inflate;
pub mod khm;
pub mod layout;
pub mod ktf;
pub mod kwb;
pub mod magic;
pub mod oid;
pub mod opl2;
pub mod pak;
pub mod pe;
pub mod quake;
pub mod png;
pub mod recipe;
pub mod salary;
pub mod save;
pub mod subject;
pub mod table;
pub mod voice;
pub mod world;
pub mod vorbis;
pub mod xml_e;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// The data does not follow the format.
    Corrupt(&'static str),
    /// The data would decode to more than the caller allowed.
    TooLarge,
    /// The data is not in this format at all.
    NotThisFormat,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Corrupt(what) => write!(f, "corrupt data: {what}"),
            Error::TooLarge => write!(f, "data larger than allowed"),
            Error::NotThisFormat => write!(f, "not this format"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn u32_at(data: &[u8], offset: usize) -> Result<u32, Error> {
    data.get(offset..offset + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Error::Corrupt("read past the end"))
}
