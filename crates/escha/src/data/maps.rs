//! The field maps, named in the executable's `FIELDMAP_*` enum.

/// The field maps by `FIELDMAP_*` number, without the prefix: the names of the field
/// directories under `Data/x64/Field` (`TOWN_01` to `TOWN_12` are 0 to 11, then `AREA_01_01`
/// and on; 122 in Escha & Logy DX), from the executable's enum names.
pub fn field_maps(pe: &barrel_formats::pe::Pe) -> Vec<String> {
    pe.enum_names("FIELDMAP_TOWN_01")
        .map(|(names, first)| {
            names[first..].iter().take_while(|n| n.starts_with("FIELDMAP_")).map(|n| n["FIELDMAP_".len()..].to_string()).collect()
        })
        .unwrap_or_default()
}
