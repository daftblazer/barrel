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
