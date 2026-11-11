//! Object-id name lists (`*Oid.bin`, one per model container): the model's node names in
//! node order (`root`, `Bip001_Pelvis`, ...), which give the skeleton's bones their names.
//!
//! The file is a run of `{u8 length, that many ASCII bytes}` ended by one 0xff byte.

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

#[cfg(test)]
mod tests {
    #[test]
    fn names() {
        assert_eq!(super::parse(b"\x04root\x03Bip\xff").unwrap(), vec!["root", "Bip"]);
        assert!(super::parse(b"\x04roo").is_err());
        assert!(super::parse(b"\x04root").is_err());
    }
}
