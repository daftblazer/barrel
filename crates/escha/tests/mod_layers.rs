//! A mod folder that holds one XML table of the game changes what that table reads, through the
//! same layered files the game's tables are read through; the game's other tables read as before.

use std::path::{Path, PathBuf};

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

#[test]
fn an_enabled_mod_replaces_one_table() {
    let base: PathBuf = std::env::temp_dir().join(format!("barrel-mod-layers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (game, mods) = (base.join("game"), base.join("mods"));
    write(&game, "Saves_EN/item/itemData.xml", r#"<root><item ItemName="Puni"/><item ItemName="Water"/></root>"#);
    write(&game, "Saves_EN/shop/ShopData.xml", r#"<root><shop Name="Plain"/></root>"#);
    write(&mods, "cheaper/mod.toml", "name = \"Cheaper\"\nversion = \"1\"\n");
    // The mod spells the folders in other case, as a Windows user's tools may.
    write(&mods, "cheaper/saves_en/ITEM/itemdata.xml", r#"<root><item ItemName="Modded Puni"/><item ItemName="Water"/></root>"#);

    let first_name = |rel: &str| {
        let t = barrel_formats::table::read(&game.join(rel)).unwrap();
        let row = &t.children[0];
        row.attr("ItemName").or(row.attr("Name")).unwrap().to_string()
    };
    // No layers installed yet: the files as they are on the disk.
    assert_eq!(first_name("Saves_EN/item/itemData.xml"), "Puni");
    // The game's folder with the mod enabled (as `mods.toml` lists it).
    assert!(barrel_files::install(barrel_files::mods::game_fs_with(&game, Some(&mods), &["cheaper".to_string()])));
    assert_eq!(first_name("Saves_EN/item/itemData.xml"), "Modded Puni");
    assert_eq!(first_name("Saves_EN/shop/ShopData.xml"), "Plain");
    // The game's own file on the disk is untouched.
    assert!(std::fs::read_to_string(game.join("Saves_EN/item/itemData.xml")).unwrap().contains("\"Puni\""));
    let _ = std::fs::remove_dir_all(base);
}
