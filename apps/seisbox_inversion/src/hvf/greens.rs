//! Surface-wave Green's function contributions.
//!
//! Translates Fortran `GR_new.f90` (Rayleigh) and `GL.f90` (Love).

use crate::hvf::model::ModelParams;
use crate::hvf::types::Float;
use crate::hvf::wang::{wang_love, wang_rayleigh};

/// Compute the Rayleigh-wave contribution of one mode to Green's functions.
///
/// Returns `(g1, img3)` vectors over frequency, and updates slowness to group velocity.
///
/// Translates Fortran `SUBROUTINE GR(G1, IMG3, INDEX, OFFSET_R)`.
///
/// # Arguments
/// * `params` — Model parameters.
/// * `omega` — Circular frequency vector.
/// * `slowness` — Phase slowness values for this mode (will be overwritten with group slowness).
/// * `offset` — Number of frequencies below cutoff for this mode.
pub fn compute_gr(
    params: &ModelParams,
    omega: &[Float],
    slowness: &mut [Float],
    offset: usize,
) -> (Vec<Float>, Vec<Float>) {
    let nfreq = omega.len();
    let mut g1 = vec![0.0_f64; nfreq];
    let mut img3 = vec![0.0_f64; nfreq];

    for j in 0..nfreq - offset {
        let freq_idx = offset + j;
        let dsr = slowness[freq_idx];
        let dw = omega[freq_idx];

        let result = wang_rayleigh(dw, dsr, params);

        // Group velocity
        let vg = 0.5 * dsr
            * (((1.0_f64 / dsr).powi(2) * (result.i1 + result.i2)
                - 1.0 / (dsr * dw).powi(2) * result.id2)
                + result.ii1)
            / (result.i1 + result.i2);

        // Overwrite phase slowness with group slowness (inverse group velocity)
        slowness[freq_idx] = 1.0 / vg;

        let ar = dsr / (2.0 * vg * (result.i1 + result.i2));

        g1[freq_idx] = -0.5 * ar * result.ellipticity.im * result.ellipticity.im;
        img3[freq_idx] = -0.5 * ar;
    }

    (g1, img3)
}

/// Compute the Love-wave contribution of one mode to the Green's function.
///
/// Returns `img2` vector over frequency, and updates slowness to group velocity.
///
/// Translates Fortran `SUBROUTINE GL(IMG2, INDEX, OFFSET_L)`.
pub fn compute_gl(
    params: &ModelParams,
    omega: &[Float],
    slowness: &mut [Float],
    offset: usize,
) -> Vec<Float> {
    let nfreq = omega.len();
    let mut img2 = vec![0.0_f64; nfreq];

    for j in 0..nfreq - offset {
        let freq_idx = offset + j;

        let result = wang_love(omega[freq_idx], slowness[freq_idx], params);

        let al = 1.0 / (2.0 * result.integral_ratio * result.integral);

        // Overwrite phase slowness with group slowness
        slowness[freq_idx] = 1.0 / (slowness[freq_idx] * result.integral_ratio);

        img2[freq_idx] = 0.5 * al;
    }

    img2
}
