//! `barrel-escha`: the game's own program, with every developer flag (`play`); the `barrel` program
//! picks the game by itself.

fn main() -> barrel_render::Result<()> {
    barrel_escha::play::main()
}
