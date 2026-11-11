//! Telling which game an install directory holds.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    AyeshaDx,
    EschaLogyDx,
    ShallieDx,
    Sophie2,
    RyzaDx,
    Ryza2Dx,
    Ryza3Dx,
}

impl Game {
    pub const ALL: [Game; 7] = [
        Game::AyeshaDx,
        Game::EschaLogyDx,
        Game::ShallieDx,
        Game::Sophie2,
        Game::RyzaDx,
        Game::Ryza2Dx,
        Game::Ryza3Dx,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Game::AyeshaDx => "Atelier Ayesha DX",
            Game::EschaLogyDx => "Atelier Escha & Logy DX",
            Game::ShallieDx => "Atelier Shallie DX",
            Game::Sophie2 => "Atelier Sophie 2",
            Game::RyzaDx => "Atelier Ryza DX",
            Game::Ryza2Dx => "Atelier Ryza 2 DX",
            Game::Ryza3Dx => "Atelier Ryza 3 DX",
        }
    }

    /// The executable the Steam release ships, which identifies the install.
    pub fn executable(self) -> &'static str {
        match self {
            Game::AyeshaDx => "Atelier_Ayesha.exe",
            Game::EschaLogyDx => "Atelier_Escha_and_Logy.exe",
            Game::ShallieDx => "Atelier_Shallie.exe",
            Game::Sophie2 => "Atelier_Sophie_2.exe",
            Game::RyzaDx => "Atelier_Ryza_DX.exe",
            Game::Ryza2Dx => "Atelier_Ryza_2_DX.exe",
            Game::Ryza3Dx => "Atelier_Ryza_3_DX.exe",
        }
    }

    pub fn detect(install: &Path) -> Option<Game> {
        Game::ALL.into_iter().find(|g| install.join(g.executable()).is_file())
    }
}
