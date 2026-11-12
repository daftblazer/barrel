FSR 3.1's upscaler shaders, vendored unchanged from AMD's FidelityFX SDK v1.1.4 (commit
c6efa6b, MIT; `LICENSE.txt`): the GPU headers under `gpu/` and the Vulkan pass entry points
under `fsr3upscaler/`. `crates/render/build.rs` compiles the passes as compute shaders with
Barrel's permutation (see `docs/engine.md`, "FSR 3.1"). The debug-view pass is left out.
