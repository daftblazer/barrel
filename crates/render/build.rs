//! Compiles the GLSL shaders to SPIR-V with glslc (from the Vulkan SDK / shaderc).

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // Included by others.
    println!("cargo:rerun-if-changed=shaders/gsr2_common.glsl");
    for name in ["model.vert", "model.frag", "terrain.frag", "text.vert", "text.frag", "movie.vert", "movie.frag", "post.vert", "post.frag", "sky.vert", "sky.frag", "glow.frag", "shadow.frag", "easu.frag", "rcas.frag", "taa.frag", "gsr1.frag", "gsr2_convert.frag", "gsr2_upscale.frag"] {
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
    // FSR 3.1's upscaler passes (shaders/ffx, vendored from the FidelityFX SDK), as compute
    // shaders with Barrel's permutation: LDR colour, render-size motion vectors without the
    // jitter, depth not inverted, the LUT Lanczos for reprojection, no sharpening inside (RCAS
    // runs as its own pass).
    println!("cargo:rerun-if-changed=shaders/ffx");
    for pass in ["prepare_inputs", "luma_pyramid", "shading_change_pyramid", "shading_change", "prepare_reactivity", "luma_instability", "accumulate", "rcas", "autogen_reactive"] {
        let src = PathBuf::from(format!("shaders/ffx/fsr3upscaler/ffx_fsr3upscaler_{pass}_pass.glsl"));
        let dst = out.join(format!("fsr3_{pass}.spv"));
        let status = Command::new(std::env::var("GLSLC").unwrap_or_else(|_| "glslc".into()))
            .args(["-O", "-fshader-stage=comp", "--target-env=vulkan1.1"])
            .args([
                "-DFFX_GPU=1",
                "-DFFX_GLSL=1",
                "-DFFX_FSR3UPSCALER_OPTION_UPSAMPLE_SAMPLERS_USE_DATA_HALF=0",
                "-DFFX_FSR3UPSCALER_OPTION_ACCUMULATE_SAMPLERS_USE_DATA_HALF=0",
                "-DFFX_FSR3UPSCALER_OPTION_REPROJECT_SAMPLERS_USE_DATA_HALF=1",
                "-DFFX_FSR3UPSCALER_OPTION_POSTPROCESSLOCKSTATUS_SAMPLERS_USE_DATA_HALF=0",
                "-DFFX_FSR3UPSCALER_OPTION_UPSAMPLE_USE_LANCZOS_TYPE=2",
                "-DFFX_FSR3UPSCALER_OPTION_REPROJECT_USE_LANCZOS_TYPE=1",
                "-DFFX_FSR3UPSCALER_OPTION_HDR_COLOR_INPUT=0",
                "-DFFX_FSR3UPSCALER_OPTION_LOW_RESOLUTION_MOTION_VECTORS=1",
                "-DFFX_FSR3UPSCALER_OPTION_JITTERED_MOTION_VECTORS=0",
                "-DFFX_FSR3UPSCALER_OPTION_INVERTED_DEPTH=0",
                "-DFFX_FSR3UPSCALER_OPTION_APPLY_SHARPENING=0",
                "-Ishaders/ffx/gpu",
                "-Ishaders/ffx/gpu/fsr3upscaler",
            ])
            .arg(&src)
            .arg("-o")
            .arg(&dst)
            .status()
            .expect("glslc not found: install shaderc (or set GLSLC)");
        assert!(status.success(), "glslc failed on {}", src.display());
    }
}
