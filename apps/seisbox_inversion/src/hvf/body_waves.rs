//! Body-wave integrals (damped wavenumber integration).
//!
//! Translates Fortran `BW_INTEGRALS_DAMPED.f90`.

use crate::hvf::matrix::*;
use crate::hvf::model::ModelParams;
use crate::hvf::types::*;

/// Result of body-wave integration at a single frequency.
#[derive(Debug, Clone, Copy)]
pub struct BodyWaveResult {
    /// ImG33 contribution from body waves (vertical).
    pub sum_v: Float,
    /// ImG11 horizontal PSV contribution.
    pub sum_psv: Float,
    /// ImG22 horizontal SH contribution.
    pub sum_sh: Float,
}

/// Compute body-wave integrals at a single frequency using complex-ω integration.
///
/// Translates Fortran `SUBROUTINE BWR(SUMV, SUMPSV, SUMSH, Nks, W)`.
///
/// # Arguments
/// * `params` — Model parameters.
/// * `nks` — Number of positive k-values for integration.
/// * `omega` — Circular frequency (real).
/// * `sh_damp` — Attenuation factor for SH: ω → ω - i·apsv·ω.
/// * `psv_damp` — Attenuation factor for PSV.
pub fn bw_integrals(
    params: &ModelParams,
    nks: usize,
    omega: Float,
    sh_damp: Float,
    psv_damp: Float,
) -> BodyWaveResult {
    let ncapas = params.nlayers;
    let w = omega;

    let cw_sh = cxri(w, -w * sh_damp);
    let cw_psv = cxri(w, -w * psv_damp);

    let dk = w / params.beta[ncapas - 1] / nks as Float;

    let mut hpsv_arr = vec![0.0_f64; nks + 1];
    let mut v_arr = vec![0.0_f64; nks + 1];
    let mut sh_arr = vec![0.0_f64; nks + 1];

    // Loop from k = w/beta_halfspace down to k = dk, then one extra point at dk*0.1
    // Fortran uses a GOTO-based loop; we restructure it here.

    // Main loop: ik from nks+1 down to 2 (0-indexed: nks down to 1)
    let mut ik = nks; // 0-indexed (corresponds to Fortran nks+1)
    let mut k = w / params.beta[ncapas - 1] + dk;

    loop {
        k -= dk;
        let (hpsv_val, v_val, sh_val) =
            bw_single_k(k, cw_sh, cw_psv, params, ncapas);
        hpsv_arr[ik] = hpsv_val;
        v_arr[ik] = v_val;
        sh_arr[ik] = sh_val;

        if ik <= 1 {
            break;
        }
        ik -= 1;
    }

    // Extra point at k = dk * 0.1 (ik = 0)
    let k_extra = dk * 0.1;
    let (hpsv_val, v_val, sh_val) =
        bw_single_k(k_extra, cw_sh, cw_psv, params, ncapas);
    hpsv_arr[0] = hpsv_val;
    v_arr[0] = v_val;
    sh_arr[0] = sh_val;

    // Integrate
    let aux1 = -1.0 / TWO_PI * dk;
    let sum_v: Float = aux1 * v_arr.iter().sum::<Float>();
    let sum_psv: Float = aux1 * 0.5 * hpsv_arr.iter().sum::<Float>();
    let sum_sh: Float = aux1 * 0.5 * sh_arr.iter().sum::<Float>();

    BodyWaveResult {
        sum_v,
        sum_psv,
        sum_sh,
    }
}

/// Compute body-wave contribution at a single wavenumber k.
fn bw_single_k(
    k: Float,
    cw_sh: Cx,
    cw_psv: Cx,
    params: &ModelParams,
    ncapas: usize,
) -> (Float, Float, Float) {
    let k2 = k * k;
    let _cc_sh = cw_sh / cx(k);
    let cc_psv = cw_psv / cx(k);

    // Halfspace wavenumbers
    let nua = (cx(k2) - (cw_psv / cx(params.alpha[ncapas - 1])).powi(2))
        .sqrt()
        .conj();
    let nub_psv = (cx(k2) - (cw_psv / cx(params.beta[ncapas - 1])).powi(2))
        .sqrt()
        .conj();
    let nub_sh = (cx(k2) - (cw_sh / cx(params.beta[ncapas - 1])).powi(2))
        .sqrt()
        .conj();

    // Build L-matrices for halfspace
    let lpsv = l_matrix_complex(
        params.alpha[ncapas - 1],
        params.beta[ncapas - 1],
        params.mu[ncapas - 1],
        -cw_psv,
        -cc_psv,
        nua,
        nub_psv,
    );
    let lsh = l_matrix_sh(params.mu[ncapas - 1], nub_sh);

    // Initial D matrices
    let mut dpsv: [Vec4; 2] = [
        [cx(1.0), cx(0.0), cx(0.0), cx(0.0)],
        [cx(0.0), cx(1.0), cx(0.0), cx(0.0)],
    ];
    let mut dsh: Vec2 = [cx(1.0), cx(0.0)];

    let mut ypsv = matmul4x2(&lpsv, &dpsv);
    let mut ysh = matvec2(&lsh, &dsh);

    // Propagate upwards through layers
    for il in (0..ncapas - 1).rev() {
        let nua = (cx(k2) - (cw_psv / cx(params.alpha[il])).powi(2))
            .sqrt()
            .conj();
        let nub_psv = (cx(k2) - (cw_psv / cx(params.beta[il])).powi(2))
            .sqrt()
            .conj();
        let nub_sh = (cx(k2) - (cw_sh / cx(params.beta[il])).powi(2))
            .sqrt()
            .conj();

        let lpsv = l_matrix_complex(
            params.alpha[il], params.beta[il], params.mu[il],
            -cw_psv, -cc_psv, nua, nub_psv,
        );
        let lsh = l_matrix_sh(params.mu[il], nub_sh);

        let lpsv_inv = l_matrix_inv_complex(
            params.alpha[il], params.beta[il], params.mu[il],
            -cw_psv, -cc_psv, nua, nub_psv,
        );
        let lsh_inv = l_matrix_sh_inv(params.mu[il], nub_sh);

        dpsv = matmul4x2(&lpsv_inv, &ypsv);
        dsh = matvec2(&lsh_inv, &ysh);

        // Normalize PSV
        let norm = 1.0 / (dot4(&dpsv[0], &dpsv[0]).re * dot4(&dpsv[1], &dpsv[1]).re).sqrt();
        let qpsv: Mat2x2 = [
            [dpsv[1][1] * cx(norm), -dpsv[1][0] * cx(norm)],
            [-dpsv[0][1] * cx(norm), dpsv[0][0] * cx(norm)],
        ];

        // Apply Q to D
        dpsv = [
            [
                dpsv[0][0] * qpsv[0][0] + dpsv[1][0] * qpsv[1][0],
                dpsv[0][1] * qpsv[0][0] + dpsv[1][1] * qpsv[1][0],
                dpsv[0][2] * qpsv[0][0] + dpsv[1][2] * qpsv[1][0],
                dpsv[0][3] * qpsv[0][0] + dpsv[1][3] * qpsv[1][0],
            ],
            [
                dpsv[0][0] * qpsv[0][1] + dpsv[1][0] * qpsv[1][1],
                dpsv[0][1] * qpsv[0][1] + dpsv[1][1] * qpsv[1][1],
                dpsv[0][2] * qpsv[0][1] + dpsv[1][2] * qpsv[1][1],
                dpsv[0][3] * qpsv[0][1] + dpsv[1][3] * qpsv[1][1],
            ],
        ];
        dpsv[1][0] = cx(0.0);
        dpsv[0][1] = cx(0.0);

        // Normalize SH
        let norm_sh = 1.0 / dot2(&dsh, &dsh).re.sqrt();
        dsh[0] *= cx(norm_sh);
        dsh[1] *= cx(norm_sh);

        // Propagate with E-matrices
        let (epsv_inv, _) = e_matrix_norma(-params.h[il], nua, nub_psv, None);
        let (esh_inv, _) = e_matrix_norma_sh(-params.h[il], nub_sh);

        ysh = matvec2(&matmul2(&lsh, &esh_inv), &dsh);
        ypsv = matmul4x2(&matmul4(&lpsv, &epsv_inv), &dpsv);
    }

    // Extract results at the surface
    let sh_val = (I * cx(k) * ysh[0] / ysh[1]).re;

    // Normalize PSV columns by their max absolute value
    let max0 = ypsv[0].iter().map(|v| v.norm()).fold(0.0_f64, Float::max);
    let max1 = ypsv[1].iter().map(|v| v.norm()).fold(0.0_f64, Float::max);
    if max0 > 0.0 {
        for r in 0..4 { ypsv[0][r] /= cx(max0); }
    }
    if max1 > 0.0 {
        for r in 0..4 { ypsv[1][r] /= cx(max1); }
    }

    let denom_psv = -ypsv[1][2] * ypsv[0][3] + ypsv[0][2] * ypsv[1][3];
    let v_val = (cx(k) * I * (ypsv[1][1] * ypsv[0][2] - ypsv[0][1] * ypsv[1][2]) / denom_psv).re;
    let hpsv_val = (I * cx(k) * (-ypsv[1][0] * ypsv[0][3] + ypsv[0][0] * ypsv[1][3]) / denom_psv).re;

    (hpsv_val, v_val, sh_val)
}
