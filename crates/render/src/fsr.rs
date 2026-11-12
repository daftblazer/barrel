//! FSR 1 (AMD FidelityFX Super Resolution 1.0, MIT licence; the notice is in `easu.frag` and
//! `rcas.frag`): the constants its two passes take, worked out on the CPU as `ffx_fsr1.h`'s
//! `FsrEasuCon` and `FsrRcasCon` do. EASU scales the scene to the output, edge-adaptively; RCAS
//! sharpens the result.


/// EASU's four constant vectors for a scene of `input` pixels drawn to `output` pixels (the
/// whole image is the viewport): output pixel to scene pixel (`con0`), scene pixel to texture
/// coordinate and the first gather's offset (`con1`), the other gathers' offsets (`con2`,
/// `con3`).
pub fn easu_constants(input: (u32, u32), output: (u32, u32)) -> [[f32; 4]; 4] {
    let (iw, ih) = (input.0 as f32, input.1 as f32);
    let (ow, oh) = (output.0 as f32, output.1 as f32);
    [
        [iw / ow, ih / oh, 0.5 * iw / ow - 0.5, 0.5 * ih / oh - 0.5],
        [1.0 / iw, 1.0 / ih, 1.0 / iw, -1.0 / ih],
        [-1.0 / iw, 2.0 / ih, 1.0 / iw, 2.0 / ih],
        [0.0, 4.0 / ih, 0.0, 0.0],
    ]
}

/// RCAS's sharpness from `stops`: 0 the sharpest, each stop halving it.
pub fn rcas_sharpness(stops: f32) -> f32 {
    (-stops).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_size_constants() {
        let c = easu_constants((640, 360), (1280, 720));
        assert_eq!(c[0], [0.5, 0.5, -0.25, -0.25]);
        assert_eq!(c[1][0], 1.0 / 640.0);
        assert_eq!(c[3][1], 4.0 / 360.0);
        assert_eq!((rcas_sharpness(0.0), rcas_sharpness(1.0)), (1.0, 0.5));
    }
}
