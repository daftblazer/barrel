//! Naming the formats found inside containers by their first four bytes.

/// A short name for the format whose data starts with `data`, if it is one Barrel knows.
pub fn identify(data: &[u8]) -> Option<&'static str> {
    let magic: [u8; 4] = data.get(..4)?.try_into().ok()?;
    Some(match &magic {
        b"_M1G" => "g1m model",
        b"_A1G" => "g1a animation",
        b"GT1G" => "g1t textures",
        b"XF1G" => "g1e effect",
        b"OC1G" => "g1co collision",
        b"_S2G" => "g1s shader",
        b"_H1G" => "g1h morph",
        b"KFTK" => "ktf terrain",
        b"_MHK" => "khm heightmap",
        b"_SPK" => "kps",
        b"CRAE" => "elixir container",
        _ => return None,
    })
}
