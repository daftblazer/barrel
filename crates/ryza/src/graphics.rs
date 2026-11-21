//! The original's own graphics settings, with the original's meaning (docs/formats.md, "Ryza DX:
//! the graphics presets"): an overall quality, and seventeen items it uses when the quality is
//! `Custom`. The original keeps them as integers in `Setting.ini`'s `[Graphics]` section; Barrel
//! also takes them from its graphics config by their level names.

/// `GRAPHICS_QUALITY_ALL`: a general preset (`generalpreset.xml`) that decides every item, or
/// `Custom`, where the items decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Middle,
    High,
    Custom,
}

impl Quality {
    const NAMES: [&'static str; 4] = ["Low", "Middle", "High", "Custom"];

    /// The general preset's name (`Type` in `generalpreset.xml`), none for `Custom`.
    pub fn general(self) -> Option<&'static str> {
        Self::NAMES.get(self as usize).copied().filter(|_| self != Quality::Custom)
    }

    fn parse(value: &str) -> Option<Quality> {
        level(value, &Self::NAMES).map(|n| [Quality::Low, Quality::Middle, Quality::High, Quality::Custom][n])
    }
}

const THREE: &[&str] = &["Low", "Middle", "High"];
const GRASS: &[&str] = &["VeryLow", "Low", "Middle"];
const SWITCH: &[&str] = &["Off", "On"];

/// One of the seventeen items. The order is the original's `Setting.ini` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// `GRAPHICS_QUALITY_TEX`: the largest mip levels left out.
    Texture,
    /// `GRAPHICS_QUALITY_SHADOW`: the shadow maps' sizes. Stored only: the PC game's maps are
    /// the same size at every level (2000 x 2400, 2000 x 2400 and 1600 x 1600 cascades, 1024
    /// for the characters'), and Barrel keeps those.
    Shadow,
    /// `GRAPHICS_QUALITY_RLR`: the screen-space reflections' march.
    Reflections,
    /// `GRAPHICS_QUALITY_EFF`: the effects' buffer size (stored, not built).
    Effects,
    /// `GRAPHICS_ANTIAILIASING` (sic): Off, Fxaa, Taa.
    AntiAliasing,
    /// `GRAPHICS_DOF`.
    DepthOfField,
    /// `GRAPHICS_AO`: the screen-space ambient occlusion.
    Occlusion,
    /// `GRAPHICS_BLOOM`.
    Bloom,
    /// `GRAPHICS_LIGHTSHAFT` (Barrel builds none).
    LightShafts,
    /// `GRAPHICS_DISPRANGE`: the objects' draw ranges.
    DrawRange,
    /// `GRAPHICS_POINTLIGHT` (stored, not built).
    PointLights,
    /// `GRAPHICS_GRASS_RANGE`.
    GrassRange,
    /// `GRAPHICS_GRASS_DENSITY`.
    GrassDensity,
    /// `GRAPHICS_TEXTURE_FILTERING`: Linear, Anisotropic.
    Filtering,
    /// `GRAPHICS_SHADOW_DRAW_DIST`: how far the sun's shadows reach.
    ShadowDistance,
    /// `GRAPHICS_WATER_QUALITY` (stored, not built).
    Water,
    /// `GRAPHICS_LOD_DIST`.
    LodDistance,
}

pub const ITEMS: usize = 17;

impl Item {
    pub const ALL: [Item; ITEMS] = [
        Item::Texture,
        Item::Shadow,
        Item::Reflections,
        Item::Effects,
        Item::AntiAliasing,
        Item::DepthOfField,
        Item::Occlusion,
        Item::Bloom,
        Item::LightShafts,
        Item::DrawRange,
        Item::PointLights,
        Item::GrassRange,
        Item::GrassDensity,
        Item::Filtering,
        Item::ShadowDistance,
        Item::Water,
        Item::LodDistance,
    ];

    /// The key in `Setting.ini`'s `[Graphics]` section.
    pub fn ini_key(self) -> &'static str {
        [
            "GRAPHICS_QUALITY_TEX",
            "GRAPHICS_QUALITY_SHADOW",
            "GRAPHICS_QUALITY_RLR",
            "GRAPHICS_QUALITY_EFF",
            "GRAPHICS_ANTIAILIASING",
            "GRAPHICS_DOF",
            "GRAPHICS_AO",
            "GRAPHICS_BLOOM",
            "GRAPHICS_LIGHTSHAFT",
            "GRAPHICS_DISPRANGE",
            "GRAPHICS_POINTLIGHT",
            "GRAPHICS_GRASS_RANGE",
            "GRAPHICS_GRASS_DENSITY",
            "GRAPHICS_TEXTURE_FILTERING",
            "GRAPHICS_SHADOW_DRAW_DIST",
            "GRAPHICS_WATER_QUALITY",
            "GRAPHICS_LOD_DIST",
        ][self as usize]
    }

    /// The key in Barrel's graphics config (the ini key lower-cased, without `GRAPHICS_`, the
    /// misspelling mended).
    pub fn config_key(self) -> &'static str {
        ["quality_tex", "quality_shadow", "quality_rlr", "quality_eff", "antialiasing", "dof", "ao", "bloom", "lightshaft", "disprange", "pointlight", "grass_range", "grass_density", "texture_filtering", "shadow_draw_dist", "water_quality", "lod_dist"][self as usize]
    }

    /// The level names, lowest first: the `Type` names of the item's table.
    pub fn levels(self) -> &'static [&'static str] {
        match self {
            Item::Texture | Item::Shadow | Item::Reflections | Item::Effects | Item::DrawRange | Item::ShadowDistance | Item::Water | Item::LodDistance => THREE,
            Item::AntiAliasing => &["Off", "Fxaa", "Taa"],
            Item::Filtering => &["Linear", "Anisotropic"],
            Item::PointLights | Item::GrassRange | Item::GrassDensity => GRASS,
            Item::DepthOfField | Item::Occlusion | Item::Bloom | Item::LightShafts => SWITCH,
        }
    }

    /// The item's attribute in `generalpreset.xml`.
    pub fn general_attr(self) -> &'static str {
        [
            "TextureType",
            "ShadowType",
            "RLRType",
            "EffectType",
            "AAType",
            "DOFType",
            "SSAOType",
            "BloomType",
            "LightShaftType",
            "DispRangeType",
            "PointLightType",
            "GrassRangeType",
            "GrassQualityType",
            "TextureFilterType",
            "ShadowDistType",
            "WaterQuality",
            "LODDistType",
        ][self as usize]
    }

    /// The item's table: its file under `saves/graphicssetting` and its element's name.
    pub fn table(self) -> (&'static str, &'static str) {
        [
            ("texturepreset.xml", "TexturePreset"),
            ("shadowpreset.xml", "ShadowPreset"),
            ("rlrpreset.xml", "RLRPreset"),
            ("effectpreset.xml", "EffectPreset"),
            ("aapreset.xml", "AAPreset"),
            ("dofpreset.xml", "DOFPreset"),
            ("ssaopreset.xml", "SSAOPreset"),
            ("bloompreset.xml", "BloomPreset"),
            ("lightshaftpreset.xml", "LightShaftPreset"),
            ("disprangepreset.xml", "DispRangePreset"),
            ("pointlightpreset.xml", "PointLightPreset"),
            ("grassrangepreset.xml", "GrassRangePreset"),
            ("grassqualitypreset.xml", "GrassQualityPreset"),
            ("texturefilterpreset.xml", "TextureFilterPreset"),
            ("shadowdistpreset.xml", "ShadowDistPreset"),
            ("waterqualitypreset.xml", "WaterQualityPreset"),
            ("loddistpreset.xml", "LODDistPreset"),
        ][self as usize]
    }

    /// A level from its ini integer or a config's level name; an integer past the table's end
    /// is the top level, as the original treats it.
    fn parse(self, value: &str) -> Option<u8> {
        level(value, self.levels()).map(|n| n as u8)
    }
}

/// A level by name (any case) or by number, a number past the end the last.
fn level(value: &str, names: &[&str]) -> Option<usize> {
    let value = value.trim();
    match value.parse::<usize>() {
        Ok(n) => Some(n.min(names.len() - 1)),
        Err(_) => names.iter().position(|n| n.eq_ignore_ascii_case(value)),
    }
}

/// The overall quality and the items' levels (an index into [`Item::levels`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub quality: Quality,
    levels: [u8; ITEMS],
    /// Whether the player set any of it (a config's game keys or an ini): if not, Barrel keeps
    /// its own anti-aliasing, filtering, occlusion, reflections and depth of field choices
    /// (`--preset`, the config's `taa`, `anisotropy`, ...) rather than the default High's.
    pub given: bool,
}

impl Default for Settings {
    /// High overall: every item at its top level but the anti-aliasing, which the game's High
    /// preset (and its Settings screen) has at FXAA.
    fn default() -> Settings {
        let mut levels = [0u8; ITEMS];
        for item in Item::ALL {
            levels[item as usize] = item.levels().len() as u8 - 1;
        }
        levels[Item::AntiAliasing as usize] = 1;
        Settings { quality: Quality::High, levels, given: false }
    }
}

impl Settings {
    /// The item's level: an index into [`Item::levels`], what `Custom` quality uses.
    pub fn level(&self, item: Item) -> usize {
        self.levels[item as usize] as usize
    }

    pub fn set(&mut self, item: Item, level: usize) {
        self.levels[item as usize] = level.min(item.levels().len() - 1) as u8;
        self.given = true;
    }

    /// Takes `Setting.ini`'s `[Graphics]` keys over these settings; other keys and sections are
    /// skipped. The original's integers, `GRAPHICS_QUALITY_ALL` 0 Low, 1 Middle, 2 High, 3
    /// Custom.
    pub fn apply_ini(&mut self, text: &str) {
        let mut graphics = false;
        for line in text.lines() {
            let line = line.trim();
            if let Some(section) = line.strip_prefix('[') {
                graphics = section.trim_end_matches(']').eq_ignore_ascii_case("Graphics");
                continue;
            }
            let Some((key, value)) = line.split_once('=').filter(|_| graphics) else { continue };
            self.apply_key(key.trim(), value, "GRAPHICS_QUALITY_ALL", Item::ini_key, str::eq_ignore_ascii_case);
        }
    }

    /// Takes the game keys of Barrel's graphics config (`quality high|middle|low|custom` and one
    /// key per item, `Item::config_key`, with the item's level names) over these settings; the
    /// config's other keys are `RenderSettings`'s.
    pub fn apply_config(&mut self, text: &str) {
        for line in text.lines() {
            let Some((key, value)) = line.trim().split_once(' ') else { continue };
            self.apply_key(key, value, "quality", Item::config_key, |a, b| a == b);
        }
    }

    /// One `key value` of either source: `names` gives the keys (the quality's, then each
    /// item's), `same` compares a key with one of them.
    fn apply_key(&mut self, key: &str, value: &str, quality_key: &str, names: fn(Item) -> &'static str, same: fn(&str, &str) -> bool) {
        if same(key, quality_key) {
            if let Some(q) = Quality::parse(value) {
                self.quality = q;
                self.given = true;
            }
        } else if let Some(item) = Item::ALL.into_iter().find(|&i| same(key, names(i)))
            && let Some(n) = item.parse(value)
        {
            self.set(item, n as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INI: &str = "[Graphics]\r\nScreenWidth=1920\r\nGRAPHICS_QUALITY_ALL=3\r\nGRAPHICS_QUALITY_TEX=1\r\nGRAPHICS_ANTIAILIASING=1\r\nGRAPHICS_DOF=0\r\nGRAPHICS_AO=0\r\nGRAPHICS_BLOOM=0\r\nGRAPHICS_GRASS_RANGE=0\r\nGRAPHICS_TEXTURE_FILTERING=0\r\nGRAPHICS_LOD_DIST=9\r\n[Input]\r\nGRAPHICS_DOF=1\r\n";

    #[test]
    fn nothing_given_is_high_everywhere_but_the_anti_aliasing_at_fxaa() {
        let s = Settings::default();
        assert_eq!((s.quality, s.given), (Quality::High, false));
        assert_eq!(s.level(Item::AntiAliasing), 1);
        assert_eq!(s.level(Item::GrassDensity), 2);
        assert_eq!(s.level(Item::DepthOfField), 1);
    }

    #[test]
    fn reads_the_ini_graphics_section() {
        let mut s = Settings::default();
        s.apply_ini(INI);
        assert_eq!((s.quality, s.given), (Quality::Custom, true));
        assert_eq!((s.level(Item::Texture), s.level(Item::AntiAliasing), s.level(Item::DepthOfField), s.level(Item::Occlusion)), (1, 1, 0, 0));
        assert_eq!((s.level(Item::GrassRange), s.level(Item::Filtering)), (0, 0));
        // an item the ini leaves out keeps its level; one past the table's end is the top level
        assert_eq!((s.level(Item::Water), s.level(Item::LodDistance)), (2, 2));
        // the `[Input]` section's key is not the graphics'
        assert_eq!(s.level(Item::DepthOfField), 0);
    }

    #[test]
    fn the_ini_overrides_the_config() {
        let mut s = Settings::default();
        s.apply_config("scale 0.75\nquality custom\nquality_tex low\nbloom off\nao on\nantialiasing Taa\n");
        assert_eq!((s.quality, s.level(Item::Texture), s.level(Item::Bloom), s.level(Item::AntiAliasing)), (Quality::Custom, 0, 0, 2));
        s.apply_ini("[Graphics]\nGRAPHICS_QUALITY_ALL=1\nGRAPHICS_BLOOM=1\n");
        // the ini's overall quality and its items win; the config's other items stay
        assert_eq!((s.quality, s.level(Item::Bloom), s.level(Item::Texture)), (Quality::Middle, 1, 0));
    }

    #[test]
    fn a_config_without_game_keys_gives_nothing() {
        let mut s = Settings::default();
        s.apply_config("scale 0.75\ntaa on\nfoliage reduced\n");
        assert!(!s.given);
    }
}
