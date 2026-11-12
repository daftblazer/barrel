//! A field's creatures under their scripts: each field enemy as a [`Creature`] with its own
//! script world (`utility.nut` loaded), running its record's first script (`ai_*_in`) while the
//! player is in its sight and its second (`ai_*_out`) otherwise, as its brain does
//! (`0x140233a27`).

use crate::creature::Creature;
use crate::squirrel::{Error, Interp, Stmt};
use std::rc::Rc;

pub struct Mob {
    pub creature: Creature,
    /// Its scripts: while the player is in sight, and while not.
    pub scripts: (Rc<Vec<Stmt>>, Rc<Vec<Stmt>>),
    vm: Interp,
}

impl Mob {
    /// A creature with its two scripts, `utility` run first in its own script world.
    pub fn new(creature: Creature, utility: &[Stmt], scripts: (Rc<Vec<Stmt>>, Rc<Vec<Stmt>>)) -> Result<Mob, Error> {
        let mut mob = Mob { creature, scripts, vm: Interp::new() };
        mob.vm.run(utility, &mut mob.creature)?;
        Ok(mob)
    }

    /// One frame: the player's place and facing given, the script for whether it is in sight
    /// runs once, then the creature's timer, motion and move go on by `dt`.
    pub fn tick(&mut self, player: [f64; 3], player_facing: f64, dt: f64) -> Result<(), Error> {
        self.creature.player = player;
        self.creature.player_facing = player_facing;
        self.vm.budget = self.vm.budget.max(100_000);
        let script = if self.creature.aware { self.scripts.0.clone() } else { self.scripts.1.clone() };
        self.vm.run(&script, &mut self.creature)?;
        self.creature.frame(dt);
        Ok(())
    }
}

/// A field area's enemies together: each a [`Mob`], with its field radius and encounter group,
/// and the player's radius, stepped a frame at a time.
pub struct Field {
    pub mobs: Vec<Mob>,
    pub radii: Vec<f64>,
    pub groups: Vec<i32>,
    pub player_radius: f64,
}

/// A field enemy touched the player: which one, and the encounter group it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Touch {
    pub enemy: usize,
    pub group: i32,
}

impl Field {
    /// The area's `enemies` with their scripts (`script(name)` gives a parsed script; a cache is
    /// the caller's), `utility.nut` run first in each one's script world, `constants` the
    /// scripts' names, `radius(enemy)` its field radius, and generators seeded per enemy.
    pub fn new(
        enemies: &[barrel_formats::battle_data::FieldEnemy],
        constants: &std::collections::HashMap<String, i64>,
        utility: &[Stmt],
        mut script: impl FnMut(&str) -> Result<Rc<Vec<Stmt>>, String>,
        radius: impl Fn(&barrel_formats::battle_data::FieldEnemy) -> f64,
        player_radius: f64,
    ) -> Result<Field, String> {
        let mut mobs = Vec::new();
        for (k, e) in enemies.iter().enumerate() {
            let mut c = Creature::from_field_enemy(e, crate::rng::Rng::from_state([1 + k as u32, 2, 3, 4]));
            c.constants = constants.clone();
            let scripts = (script(&e.ai.0)?, script(&e.ai.1)?);
            mobs.push(Mob::new(c, utility, scripts).map_err(|e| e.to_string())?);
        }
        Ok(Field { mobs, radii: enemies.iter().map(&radius).collect(), groups: enemies.iter().map(|e| e.group).collect(), player_radius })
    }

    /// One frame: the touch check against where the enemies stand (`0x140270cc0`; a touch ends
    /// the frame there, as the field's state then turns to the encounter), else each enemy's tick.
    pub fn step(&mut self, player: [f64; 3], facing: f64, dt: f64) -> Result<Option<Touch>, String> {
        let placed: Vec<([f64; 3], f64)> = self.mobs.iter().zip(&self.radii).map(|(m, &r)| (m.creature.position, r)).collect();
        if let Some(enemy) = crate::encounter::touched(player, self.player_radius, &placed) {
            return Ok(Some(Touch { enemy, group: self.groups[enemy] }));
        }
        for (k, m) in self.mobs.iter_mut().enumerate() {
            m.tick(player, facing, dt).map_err(|e| format!("enemy {k}: {e}"))?;
        }
        Ok(None)
    }
}
