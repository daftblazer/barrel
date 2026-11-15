//! Movie playback for the games' `.wmv` files (ASF, `barrel_formats::asf`): a native VC-1
//! Advanced profile video decoder ([`vc1`]) and, later, WMA 9 Pro sound, decoded on the CPU.

pub mod bitplane;
pub mod bits;
pub mod vc1;
pub mod vc1_intra;
pub mod vc1_small;
pub mod vc1_loop;
pub mod vc1_mc;
pub mod vc1_mv;
pub mod vc1_tables;
