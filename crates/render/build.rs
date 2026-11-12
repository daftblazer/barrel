//! Compiles the GLSL shaders to SPIR-V with glslc (from the Vulkan SDK / shaderc).

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // Included by others.
    println!("cargo:rerun-if-changed=shaders/gsr2_common.glsl");
    for name in ["model.vert", "model.frag", "terrain.frag", "text.vert", "text.frag", "post.vert", "post.frag", "glow.frag", "shadow.frag", "easu.frag", "rcas.frag", "taa.frag", "gsr1.frag", "gsr2_convert.frag", "gsr2_upscale.frag"] {
        let src = PathBuf::from("shaders").join(name);
        println!("cargo:rerun-if-changed={}", src.display());
        let dst = out.join(format!("{name}.spv"));
        let status = Command::new(std::env::var("GLSLC").unwrap_or_else(|_| "glslc".into()))
            .arg("-O")
            .arg(&src)
            .arg("-o")
            .arg(&dst)
            .status()
            .expect("glslc not found: install shaderc (or set GLSLC)");
        assert!(status.success(), "glslc failed on {}", src.display());
    }
}
