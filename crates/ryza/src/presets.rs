//! The DX release's graphics presets (`saves/graphicssetting/*.xml`, docs/formats.md, "the
//! graphics presets"): the general preset names each table's type, and Barrel takes the
//! values from those tables.

use barrel_formats::table::{self, Element};

use barrel_render::reflect::Rlr;
use barrel_render::settings::RenderSettings;

use crate::graphics::{Item, Quality, Settings};
use crate::grass;

/// The anti-aliasing the game offers (`aapreset.xml`, `Value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aa {
    Off,
    Fxaa,
    Taa,
}

/// The values Barrel draws with, from the game's graphics settings through its tables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Graphics {
    /// The plants (`GrassQualityType`, `GrassRangeType`).
    pub grass: grass::Preset,
    /// Where an object's full model gives way to its middle level of detail, and that to its
    /// low one, from the eye (`LODDistType`: `Lod_hi_dist`, `Lod_mid_dist`).
    pub lod: [f32; 2],
    // STAND-IN: Levels of detail are used below the top `LODDistType` only; the real thing is the game's own switch (at High its capture draws full models at 45,000 to 78,000 units, past both distances).
    /// Whether levels of detail are used at all.
    pub lods: bool,
    /// How far the near, middle and far field objects are drawn (`DispRangeType`: `ObjNear`,
    /// `ObjMiddle`, `ObjFar`), in place of the field's own (`camera::DrawRanges::preset`).
    pub objects: [f32; 3],
    /// Whether objects have draw ranges at all: not at the top setting (`DispRangeType`
    /// `High`), where the PC game draws every object whatever its distance (`0x14011a32c`).
    pub ranged: bool,
    /// A scale of how far the sun's shadows reach (`ShadowDistType`: `Distance`, 1 on High).
    pub shadow: f32,
    /// How many of each texture's largest mip levels are left out (`TextureType`:
    /// `TextureMipBias`, 0 on High).
    pub texture_bias: usize,
    /// The screen-space ambient occlusion (`SSAOType`: `Enable`).
    pub ssao: bool,
    /// The depth of field (`DOFType`: `Enable`).
    pub dof: bool,
    /// The glow and the stars (`BloomType`: `Enable`).
    pub bloom: bool,
    /// The light shafts (`LightShaftType`: `Enable`). Barrel builds none.
    pub light_shafts: bool,
    /// The reflections' march (`RLRType`: `MaxRaySamples`, `PixelsPerStep`).
    pub rlr: Rlr,
    /// The anti-aliasing (`AAType`).
    pub aa: Aa,
    /// Anisotropic texture filtering, not linear (`TextureFilterType`).
    pub anisotropic: bool,
    /// How far point lights are gathered (`PointLightType`: `ClusterFarLimitZ`). Stored: Barrel
    /// has no point lights' cluster limit.
    pub point_light_far: f32,
    /// The water's shore bubbles and caustics (`WaterQuality`: `EnableShoreBubble`,
    /// `EnableCaustics`). Stored: Barrel builds neither.
    pub water: [bool; 2],
    /// The effects' buffer reduction (`EffectType`: `ReducedBufferLevel`). Stored: Barrel draws
    /// effects at one size.
    pub effect_level: u32,
}

impl Graphics {
    /// The game's `High` general preset, as shipped.
    pub const HIGH: Graphics = Graphics {
        grass: grass::Preset::MIDDLE,
        lod: [10000.0, 30000.0],
        lods: false,
        objects: [8000.0, 20000.0, 40000.0],
        ranged: false,
        shadow: 1.0,
        texture_bias: 0,
        ssao: true,
        dof: true,
        bloom: true,
        light_shafts: true,
        rlr: Rlr::HIGH,
        aa: Aa::Taa,
        anisotropic: true,
        point_light_far: 100000.0,
        water: [true, true],
        effect_level: 2,
    };

    /// The general preset `general` ("High", "Middle", "Low"), its tables read through `read`
    /// (a file name under `saves/graphicssetting`).
    pub fn read(general: &str, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Graphics, String> {
        let quality = [Quality::Low, Quality::Middle, Quality::High].into_iter().find(|q| q.general() == Some(general)).ok_or_else(|| format!("no general preset {general}"))?;
        let mut settings = Settings::default();
        settings.quality = quality;
        Graphics::resolve(&settings, read)
    }

    /// What `settings` ask for, through the tables: each item's type is the general preset's
    /// for it, or the item's own level when the quality is `Custom`, and the item's table gives
    /// the values for that type.
    pub fn resolve(settings: &Settings, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Graphics, String> {
        let file = |n: &str| read(n).ok_or_else(|| format!("no {n}"));
        let general = match settings.quality.general() {
            Some(name) => Some(element(&file("generalpreset.xml")?, "GeneralPreset", name)?),
            None => None,
        };
        let kind = |item: Item| -> Result<String, String> {
            match &general {
                Some(g) => g.attr(item.general_attr()).map(str::to_owned).ok_or_else(|| format!("GeneralPreset: no {}", item.general_attr())),
                None => Ok(item.levels()[settings.level(item)].to_owned()),
            }
        };
        let row = |item: Item| -> Result<Element, String> {
            let (table, tag) = item.table();
            element(&file(table)?, tag, &kind(item)?)
        };
        // A switch's table has `Off` and `On`; a type it lacks (the High preset's "Middle"
        // depth of field) is on, as any type that is not `Off`.
        let switch = |item: Item| -> Result<bool, String> {
            match row(item) {
                Ok(e) => Ok(number(&e, "Enable")? != 0.0),
                Err(_) => Ok(kind(item)? != "Off"),
            }
        };
        let (grass_density, grass_range, lod, disp, shadow, texture) = (row(Item::GrassDensity)?, row(Item::GrassRange)?, row(Item::LodDistance)?, row(Item::DrawRange)?, row(Item::ShadowDistance)?, row(Item::Texture)?);
        let (rlr, aa, filter, point, water, effects) = (row(Item::Reflections)?, row(Item::AntiAliasing)?, row(Item::Filtering)?, row(Item::PointLights)?, row(Item::Water)?, row(Item::Effects)?);
        Ok(Graphics {
            grass: grass::Preset { density: number(&grass_density, "Density")?, far: [number(&grass_range, "FarClipBegin")?, number(&grass_range, "FarClipEnd")?] },
            lod: [number(&lod, "Lod_hi_dist")?, number(&lod, "Lod_mid_dist")?],
            lods: kind(Item::LodDistance)? != "High",
            objects: [number(&disp, "ObjNear")?, number(&disp, "ObjMiddle")?, number(&disp, "ObjFar")?],
            ranged: kind(Item::DrawRange)? != "High",
            shadow: number(&shadow, "Distance")?,
            texture_bias: number(&texture, "TextureMipBias")? as usize,
            ssao: switch(Item::Occlusion)?,
            dof: switch(Item::DepthOfField)?,
            bloom: switch(Item::Bloom)?,
            light_shafts: switch(Item::LightShafts)?,
            rlr: Rlr { max_ray_samples: number(&rlr, "MaxRaySamples")? as u32, pixels_per_step: number(&rlr, "PixelsPerStep")? as u32 },
            aa: match number(&aa, "Value")? as u32 {
                0 => Aa::Off,
                1 => Aa::Fxaa,
                _ => Aa::Taa,
            },
            anisotropic: number(&filter, "TextureFilterType")? != 0.0,
            point_light_far: number(&point, "ClusterFarLimitZ")?,
            water: [number(&water, "EnableShoreBubble")? != 0.0, number(&water, "EnableCaustics")? != 0.0],
            effect_level: number(&effects, "ReducedBufferLevel")? as u32,
        })
    }

    /// `base` with these settings' passes switched: the occlusion, the depth of field, the
    /// reflections' march, the anti-aliasing and the texture filtering. The reflections run at
    /// every level.
    pub fn apply(&self, base: RenderSettings) -> RenderSettings {
        // STAND-IN: The game's FXAA (the Middle preset's anti-aliasing) is drawn as no anti-aliasing; Barrel has no FXAA.
        let taa = self.aa == Aa::Taa;
        let temporal = base.gsr2 || base.fsr3;
        RenderSettings {
            occlusion: self.ssao,
            depth_of_field: self.dof,
            reflections: true,
            rlr: self.rlr,
            taa,
            motion: taa || temporal,
            jitter: taa || temporal,
            // The game's Anisotropic is 16x; a count Barrel's page chose (config `anisotropy`) stands.
            anisotropy: if !self.anisotropic { 1 } else if base.anisotropy > 1 { base.anisotropy } else { 16 },
            ..base
        }
    }
}

/// The `tag` element of type `kind` for every field (`Field="Default"`) in a preset table.
pub fn element(bytes: &[u8], tag: &str, kind: &str) -> Result<Element, String> {
    let root = table::parse(bytes).map_err(|e| format!("{tag}: {e}"))?;
    let mut all = vec![&root];
    while let Some(e) = all.pop() {
        if e.name == tag && e.attr("Type") == Some(kind) && e.attr("Field").is_none_or(|f| f == "Default") {
            return Ok(e.clone());
        }
        all.extend(e.children.iter());
    }
    Err(format!("{tag}: no {kind}"))
}

fn number(e: &Element, k: &str) -> Result<f32, String> {
    e.attr(k).and_then(|v| v.trim().parse::<f32>().ok()).ok_or_else(|| format!("{}: no {k}", e.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::Item;

    /// The shipped tables' rows, trimmed to the attributes Barrel reads (and a field's own row
    /// the default must not take).
    fn tables(n: &str) -> Option<Vec<u8>> {
        let general = |t: &str, a: &str| format!(r#"<GeneralPreset Field="Default" Type="{t}" {a}/>"#);
        let xml = match n {
            "generalpreset.xml" => format!(
                "<Root>{}{}{}</Root>",
                general("High", r#"AAType="Taa" BloomType="On" DispRangeType="High" DOFType="Middle" EffectType="High" GrassRangeType="Middle" GrassQualityType="Middle" LightShaftType="On" PointLightType="Middle" RLRType="High" ShadowType="High" SSAOType="On" TextureType="High" ShadowDistType="High" WaterQuality="High" LODDistType="High" TextureFilterType="Anisotropic""#),
                general("Middle", r#"AAType="Fxaa" BloomType="On" DispRangeType="Middle" DOFType="Off" EffectType="Middle" GrassRangeType="Low" GrassQualityType="Low" LightShaftType="Off" PointLightType="Low" RLRType="Middle" ShadowType="Middle" SSAOType="On" TextureType="Middle" ShadowDistType="Middle" WaterQuality="Middle" LODDistType="Middle" TextureFilterType="Anisotropic""#),
                general("Low", r#"AAType="Off" BloomType="Off" DispRangeType="Low" DOFType="Off" EffectType="Low" GrassRangeType="VeryLow" GrassQualityType="VeryLow" LightShaftType="Off" PointLightType="VeryLow" RLRType="Low" ShadowType="Low" SSAOType="Off" TextureType="Low" ShadowDistType="Low" WaterQuality="Low" LODDistType="Low" TextureFilterType="Linear""#)
            ),
            "grassqualitypreset.xml" => r#"<Root><GrassQualityPreset Field="Default" Type="Middle" Density="1"/><GrassQualityPreset Field="Default" Type="Low" Density="0.75"/><GrassQualityPreset Field="Default" Type="VeryLow" Density="0.5"/></Root>"#.into(),
            "grassrangepreset.xml" => r#"<Root><GrassRangePreset Field="Default" Type="Middle" FarClipBegin="8000" FarClipEnd="15000"/><GrassRangePreset Field="Default" Type="Low" FarClipBegin="3000" FarClipEnd="3100"/><GrassRangePreset Field="Default" Type="VeryLow" FarClipBegin="2000" FarClipEnd="2100"/></Root>"#.into(),
            "loddistpreset.xml" => r#"<Root><LODDistPreset Field="Default" Type="High" Lod_hi_dist="10000" Lod_mid_dist="30000"/><LODDistPreset Field="OTHER" Type="Middle" Lod_hi_dist="1" Lod_mid_dist="2"/><LODDistPreset Field="Default" Type="Middle" Lod_hi_dist="8000" Lod_mid_dist="25000"/><LODDistPreset Field="Default" Type="Low" Lod_hi_dist="6000" Lod_mid_dist="20000"/></Root>"#.into(),
            "disprangepreset.xml" => r#"<Root><DispRangePreset Field="Default" Type="High" ObjFar="40000" ObjMiddle="20000" ObjNear="8000"/><DispRangePreset Field="Default" Type="Middle" ObjFar="30000" ObjMiddle="15000" ObjNear="7000"/><DispRangePreset Field="Default" Type="Low" ObjFar="20000" ObjMiddle="10000" ObjNear="5000"/></Root>"#.into(),
            "shadowdistpreset.xml" => r#"<Root><ShadowDistPreset Field="Default" Type="High" Distance="1"/><ShadowDistPreset Field="Default" Type="Middle" Distance="0.7"/><ShadowDistPreset Field="Default" Type="Low" Distance="0.5"/></Root>"#.into(),
            "texturepreset.xml" => r#"<Root><TexturePreset Field="Default" Type="High" TextureMipBias="0"/><TexturePreset Field="Default" Type="Middle" TextureMipBias="1"/><TexturePreset Field="Default" Type="Low" TextureMipBias="2"/></Root>"#.into(),
            "rlrpreset.xml" => r#"<Root><RLRPreset Field="Default" Type="High" MaxRaySamples="256" PixelsPerStep="1"/><RLRPreset Field="Default" Type="Middle" MaxRaySamples="128" PixelsPerStep="2"/><RLRPreset Field="Default" Type="Low" MaxRaySamples="64" PixelsPerStep="4"/></Root>"#.into(),
            "aapreset.xml" => r#"<Root><AAPreset Field="Default" Type="Off" Value="0"/><AAPreset Field="Default" Type="Fxaa" Value="1"/><AAPreset Field="Default" Type="Taa" Value="2"/></Root>"#.into(),
            "texturefilterpreset.xml" => r#"<Root><TextureFilterPreset Field="Default" Type="Linear" TextureFilterType="0"/><TextureFilterPreset Field="Default" Type="Anisotropic" TextureFilterType="1"/></Root>"#.into(),
            "pointlightpreset.xml" => r#"<Root><PointLightPreset Field="Default" Type="Middle" ClusterFarLimitZ="100000"/><PointLightPreset Field="Default" Type="Low" ClusterFarLimitZ="50000"/><PointLightPreset Field="Default" Type="VeryLow" ClusterFarLimitZ="10000"/></Root>"#.into(),
            "waterqualitypreset.xml" => r#"<Root><WaterQualityPreset Field="Default" Type="High" EnableShoreBubble="1" EnableCaustics="1"/><WaterQualityPreset Field="Default" Type="Middle" EnableShoreBubble="0" EnableCaustics="1"/><WaterQualityPreset Field="Default" Type="Low" EnableShoreBubble="0" EnableCaustics="0"/></Root>"#.into(),
            "effectpreset.xml" => r#"<Root><EffectPreset Field="Default" Type="High" ReducedBufferLevel="2"/><EffectPreset Field="Default" Type="Middle" ReducedBufferLevel="1"/><EffectPreset Field="Default" Type="Low" ReducedBufferLevel="0"/></Root>"#.into(),
            f @ ("ssaopreset.xml" | "bloompreset.xml" | "dofpreset.xml" | "lightshaftpreset.xml") => {
                let tag = match f {
                    "ssaopreset.xml" => "SSAOPreset",
                    "bloompreset.xml" => "BloomPreset",
                    "dofpreset.xml" => "DOFPreset",
                    _ => "LightShaftPreset",
                };
                format!(r#"<Root><{tag} Field="Default" Type="Off" Enable="0"/><{tag} Field="Default" Type="On" Enable="1"/></Root>"#)
            }
            _ => return None,
        };
        Some(xml.into_bytes())
    }

    #[test]
    fn reads_a_general_preset_through_its_tables() {
        assert_eq!(Graphics::read("High", &tables).unwrap(), Graphics::HIGH);
        let middle = Graphics::read("Middle", &tables).unwrap();
        assert_eq!(
            (middle.grass, middle.lod, middle.lods, middle.objects, middle.ranged, middle.shadow, middle.texture_bias),
            (grass::Preset { density: 0.75, far: [3000.0, 3100.0] }, [8000.0, 25000.0], true, [7000.0, 15000.0, 30000.0], true, 0.7, 1)
        );
        assert_eq!((middle.aa, middle.ssao, middle.dof, middle.bloom, middle.light_shafts), (Aa::Fxaa, true, false, true, false));
        assert_eq!(middle.rlr, Rlr { max_ray_samples: 128, pixels_per_step: 2 });
        assert!(Graphics::read("Nope", &tables).is_err());
        assert!(Graphics::read("High", &|_| None).is_err());
    }

    #[test]
    fn low_has_no_passes_but_the_reflections() {
        let low = Graphics::read("Low", &tables).unwrap();
        assert_eq!((low.aa, low.ssao, low.dof, low.bloom, low.light_shafts, low.anisotropic), (Aa::Off, false, false, false, false, false));
        assert_eq!((low.rlr, low.texture_bias, low.point_light_far, low.water, low.effect_level), (Rlr { max_ray_samples: 64, pixels_per_step: 4 }, 2, 10000.0, [false, false], 0));
        let drawn = low.apply(RenderSettings::DESKTOP);
        assert!(drawn.reflections && !drawn.occlusion && !drawn.depth_of_field && !drawn.taa);
        assert_eq!((drawn.anisotropy, drawn.rlr), (1, low.rlr));
        // Barrel's TAA follows the game's `Taa` only
        assert!(Graphics::HIGH.apply(RenderSettings::DESKTOP).taa);
    }

    #[test]
    fn the_games_anisotropic_keeps_the_count_barrels_page_chose() {
        let mut base = RenderSettings::DESKTOP;
        base.anisotropy = 4;
        assert_eq!(Graphics::HIGH.apply(base).anisotropy, 4);
        // Nothing chosen (1): the game's own 16x; Linear is 1 whatever the count.
        base.anisotropy = 1;
        assert_eq!(Graphics::HIGH.apply(base).anisotropy, 16);
        base.anisotropy = 4;
        assert_eq!(Graphics::read("Low", &tables).unwrap().apply(base).anisotropy, 1);
    }

    #[test]
    fn custom_takes_each_table_by_its_own_item() {
        // The handheld's: texture Middle, FXAA, everything else at its lowest.
        let mut s = Settings::default();
        s.apply_ini("[Graphics]\nGRAPHICS_QUALITY_ALL=3\nGRAPHICS_QUALITY_TEX=1\nGRAPHICS_ANTIAILIASING=1\nGRAPHICS_QUALITY_SHADOW=0\nGRAPHICS_QUALITY_RLR=0\nGRAPHICS_QUALITY_EFF=0\nGRAPHICS_DOF=0\nGRAPHICS_AO=0\nGRAPHICS_BLOOM=0\nGRAPHICS_LIGHTSHAFT=0\nGRAPHICS_DISPRANGE=0\nGRAPHICS_POINTLIGHT=0\nGRAPHICS_GRASS_RANGE=0\nGRAPHICS_GRASS_DENSITY=0\nGRAPHICS_TEXTURE_FILTERING=0\nGRAPHICS_SHADOW_DRAW_DIST=0\nGRAPHICS_WATER_QUALITY=0\nGRAPHICS_LOD_DIST=0\n");
        let g = Graphics::resolve(&s, &tables).unwrap();
        let low = Graphics::read("Low", &tables).unwrap();
        assert_eq!(Graphics { texture_bias: 1, aa: Aa::Fxaa, ..low }, g);
        // Each item decides its own table: only the shadow distance raised here.
        s.set(Item::ShadowDistance, 2);
        assert_eq!(Graphics::resolve(&s, &tables).unwrap().shadow, 1.0);
        // The overall quality overrides the items: Low with every item high is Low.
        let mut all = Settings::default();
        all.apply_ini("[Graphics]\nGRAPHICS_QUALITY_ALL=0\n");
        assert_eq!(Graphics::resolve(&all, &tables).unwrap(), low);
    }
}
