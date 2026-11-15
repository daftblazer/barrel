//! Ryza's field map numbers (`FIELDMAP_*`, the events' `q2` / `q12` operand and the
//! `FieldChange` steps): the executable's enum name array, in number order, from
//! `FIELDMAP_S01_I_ATTIC` (0) on. A field's container is its name in lower case without the
//! prefix (`FIELDMAP_S06_T_RASEN` is `s06_t_rasen`).

/// The field names by number, lower-cased as the containers are named.
pub fn field_maps(pe: &barrel_formats::pe::Pe) -> Vec<String> {
    pe.enum_names("FIELDMAP_S01_I_ATTIC")
        .map(|(names, first)| names[first..].iter().take_while(|n| n.starts_with("FIELDMAP_")).map(|n| n["FIELDMAP_".len()..].to_ascii_lowercase()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    /// The installed executable (`BARREL_RYZA_EXE`): the opening's field 5 is Rasenboden, its
    /// attic event's 0 the attic.
    #[test]
    fn the_installs_field_maps() {
        let Some(p) = std::env::var_os("BARREL_RYZA_EXE") else { return };
        let data = barrel_files::read(p).unwrap();
        let pe = barrel_formats::pe::Pe::parse(&data).unwrap();
        let maps = super::field_maps(&pe);
        assert_eq!(maps[0], "s01_i_attic");
        assert_eq!(maps[5], "s06_t_rasen");
        eprintln!("{} fields", maps.len());
        assert_eq!(maps.last().map(String::as_str), Some("s00_common_model"));
    }
}
