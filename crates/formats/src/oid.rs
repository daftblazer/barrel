//! Object-id name lists (`*Oid.bin`, one per model container): the model's node names in
//! node order (`root`, `Bip001_Pelvis`, ...), which give the skeleton's bones their names.
//!
//! The file is a run of `{u8 length, that many ASCII bytes}` ended by one 0xff byte. The Ryza
//! games' lists start with `HeaderCharaOid`, `ObjectID:<model>` and a version, then name each
//! bone with its id (`952,SWING500_Hair000`): `by_id` reads those.

use crate::Error;

pub fn parse(data: &[u8]) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    let mut p = 0;
    loop {
        match data.get(p) {
            Some(0xff) if p + 1 == data.len() => return Ok(names),
            Some(&len) => {
                let s = data.get(p + 1..p + 1 + len as usize).ok_or(Error::Corrupt("oid name past the end"))?;
                if !s.iter().all(|c| (32..127).contains(c)) {
                    return Err(Error::Corrupt("oid name is not ASCII"));
                }
                names.push(String::from_utf8_lossy(s).into_owned());
                p += 1 + len as usize;
            }
            None => return Err(Error::Corrupt("oid list has no terminator")),
        }
    }
}

/// The names given as `id,name` (the Ryza games' lists), by id.
pub fn by_id(names: &[String]) -> std::collections::HashMap<u32, &str> {
    names.iter().filter_map(|n| n.split_once(',')).filter_map(|(id, name)| Some((id.parse().ok()?, name))).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids() {
        let names = ["HeaderCharaOid", "ObjectID:PC00A_MODEL", "1", "0,root", "952,SWING500_Hair000"].map(String::from);
        let m = super::by_id(&names);
        assert_eq!((m.len(), m[&952]), (2, "SWING500_Hair000"));
    }

    #[test]
    fn names() {
        assert_eq!(super::parse(b"\x04root\x03Bip\xff").unwrap(), vec!["root", "Bip"]);
        assert!(super::parse(b"\x04roo").is_err());
        assert!(super::parse(b"\x04root").is_err());
    }
}
