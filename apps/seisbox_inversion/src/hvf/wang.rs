//! Wang propagator method for computing energy integrals and ellipticity.
//!
//! Translates Fortran `WangMethod_Rayleigh_new.f90` and `WangMethod_Love.f90`.

use crate::hvf::matrix::*;
use crate::hvf::model::ModelParams;
use crate::hvf::types::*;

// =============================================================================
// Rayleigh waves — Wang method
// =============================================================================

/// Result of the Wang method for Rayleigh waves at a single (ω, slowness).
pub struct WangRayleighResult {
    /// Ellipticity: ratio u0/w0 (horizontal/vertical displacement).
    pub ellipticity: Cx,
    /// Energy integral I1.
    pub i1: Float,
    /// Energy integral I2.
    pub i2: Float,
    /// Integral II1 (related to group velocity).
    pub ii1: Float,
    /// Integral ID2 (derivative integral).
    pub id2: Float,
}

/// Implements the improved Wang method for Rayleigh waves.
///
/// Computes energy integrals and ellipticity for a single mode at frequency ω
/// and slowness.
///
/// Translates Fortran `SUBROUTINE WangMethod_R(elip, Integral1, Integral2, IntegralI1, IntegralD2, W, SLOW)`.
pub fn wang_rayleigh(w: Float, slow: Float, params: &ModelParams) -> WangRayleighResult {
    let ncapas = params.nlayers;
    let c = 1.0 / slow;

    // Allocate per-layer storage
    let mut l_layers: Vec<Mat4x4> = vec![zero4x4(); ncapas];
    let mut aux_mtrx: Vec<[Mat4x4; 6]> = vec![[zero4x4(); 6]; ncapas];
    let mut expos_einv_int: Vec<[Float; 3]> = vec![[0.0; 3]; ncapas];
    let mut expos_qacum: Vec<Float> = vec![0.0; ncapas];
    let mut qacum_layers: Vec<Mat2x2> = vec![eye2x2(); ncapas];

    let mut expo_prov1: Float = 0.0;
    let mut expo_prov2: Float = 0.0;

    // Start from halfspace
    let idx = ncapas - 1;
    let gam = gamma_from_slow(slow, w, params.slow_p[idx]);
    let nu = nu_from_slow(slow, w, params.slow_s[idx]);
    l_layers[idx] = l_matrix(params.alpha[idx], params.beta[idx], params.mu[idx], w, c, gam, nu);

    // D = I(4×2)
    let mut ymat: [Vec4; 2] = [
        [cx(1.0), cx(0.0), cx(0.0), cx(0.0)],
        [cx(0.0), cx(1.0), cx(0.0), cx(0.0)],
    ];
    ymat = matmul4x2(&l_layers[idx], &ymat);

    // Halfspace AuxMtrx initialization
    aux_mtrx[idx][0][0][0] = cx(-1.0) / (-gam + (-gam).conj());
    aux_mtrx[idx][1][1][1] = cx(-1.0) / (-nu + (-nu).conj());
    aux_mtrx[idx][2][1][0] = cx(-1.0) / (-nu + (-gam).conj());

    aux_mtrx[idx][3][0][0] = aux_mtrx[idx][0][0][0] * (-gam * (-gam).conj());
    aux_mtrx[idx][4][1][1] = aux_mtrx[idx][1][1][1] * (-nu * (-nu).conj());
    aux_mtrx[idx][5][1][0] = aux_mtrx[idx][2][1][0] * (-nu * (-gam).conj());

    // D for halfspace is identity, so outer products are trivial
    let d_col0: Vec4 = [cx(1.0), cx(0.0), cx(0.0), cx(0.0)];
    let d_col1: Vec4 = [cx(0.0), cx(1.0), cx(0.0), cx(0.0)];

    for t in 0..6 {
        let (a, b) = match t {
            0 | 3 => (&d_col0, &d_col0),
            1 | 4 => (&d_col1, &d_col1),
            2 | 5 => (&d_col1, &d_col0),
            _ => unreachable!(),
        };
        aux_mtrx[idx][t] = hadamard4(&outer4(a, b), &aux_mtrx[idx][t]);
    }

    // Propagate upwards through layers
    for idx in (0..ncapas - 1).rev() {
        let gam = gamma_from_slow(slow, w, params.slow_p[idx]);
        let nu = nu_from_slow(slow, w, params.slow_s[idx]);
        l_layers[idx] = l_matrix(params.alpha[idx], params.beta[idx], params.mu[idx], w, c, gam, nu);
        let linv = l_matrix_inv(params.alpha[idx], params.beta[idx], params.mu[idx], w, c, gam, nu);

        let d = matmul4x2(&linv, &ymat);
        let norm = 1.0 / (dot4(&d[0], &d[0]).re * dot4(&d[1], &d[1]).re).sqrt();
        let q: Mat2x2 = [
            [d[1][1] * cx(norm), -d[1][0] * cx(norm)],
            [-d[0][1] * cx(norm), d[0][0] * cx(norm)],
        ];

        // Apply Q to D
        let mut d_new: [Vec4; 2] = [
            [
                d[0][0] * q[0][0] + d[1][0] * q[1][0],
                d[0][1] * q[0][0] + d[1][1] * q[1][0],
                d[0][2] * q[0][0] + d[1][2] * q[1][0],
                d[0][3] * q[0][0] + d[1][3] * q[1][0],
            ],
            [
                d[0][0] * q[0][1] + d[1][0] * q[1][1],
                d[0][1] * q[0][1] + d[1][1] * q[1][1],
                d[0][2] * q[0][1] + d[1][2] * q[1][1],
                d[0][3] * q[0][1] + d[1][3] * q[1][1],
            ],
        ];
        d_new[1][0] = cx(0.0);
        d_new[0][1] = cx(0.0);

        // Update Q scaling for expo
        let mut q_scaled = q;
        let scale = (expo_prov2 - expo_prov1).exp();
        q_scaled[0][0] *= cx(scale);
        q_scaled[0][1] *= cx(scale);

        // Update Qacum for layers above
        for il in (idx + 1..ncapas).rev() {
            qacum_layers[il] = matmul2(&qacum_layers[il], &q_scaled);
            expos_qacum[il] -= expo_prov2;
        }

        // Propagate with E-matrix
        let (einv1, ep1) = e_matrix_norma(-params.h[idx], gam, nu, Some(1));
        expo_prov1 = ep1;
        let col0 = matvec4(&matmul4(&l_layers[idx], &einv1), &d_new[0]);

        let (mut einv2, ep2) = e_matrix_norma(-params.h[idx], gam, nu, Some(2));
        expo_prov2 = ep2;
        einv2[0][0] = cx(0.0);
        let col1 = matvec4(&matmul4(&l_layers[idx], &einv2), &d_new[1]);

        ymat = [col0, col1];

        // Compute AuxMtrx for integrals
        let diag_einv1 = diag4(&einv1);
        let diag_einv2 = [cx(0.0), einv2[1][1], einv2[2][2], einv2[3][3]];

        let mut am1 = outer4(&diag_einv1, &conj4(&diag_einv1));
        let mut am2 = outer4(&diag_einv2, &conj4(&diag_einv2));
        let mut am3 = outer4(&diag_einv2, &conj4(&diag_einv1));

        let sub1 = cx((-2.0_f64 * expo_prov1).exp());
        let sub2 = cx((-2.0_f64 * expo_prov2).exp());
        let sub3 = cx((-expo_prov1 - expo_prov2).exp());
        for i in 0..4 {
            for j in 0..4 {
                am1[i][j] -= sub1;
                am2[i][j] -= sub2;
                am3[i][j] -= sub3;
            }
        }

        expos_einv_int[idx] = [2.0 * expo_prov1, 2.0 * expo_prov2, expo_prov1 + expo_prov2];

        // Denominators
        let mut deno = zero4x4();
        deno[0][0] = cx(-2.0 * gam.re);
        deno[0][1] = -gam - nu.conj();
        deno[0][2] = cx(-2.0 * gam.im) * I;
        deno[0][3] = -gam + nu.conj();
        deno[1][0] = deno[0][1].conj();
        deno[1][1] = cx(-2.0 * nu.re);
        deno[1][2] = -deno[0][3].conj();
        deno[1][3] = cx(-2.0 * nu.im) * I;
        deno[2][0] = -deno[0][2];
        deno[2][1] = -deno[0][3];
        deno[2][2] = -deno[0][0];
        deno[2][3] = -deno[0][1];
        deno[3][0] = -deno[1][2];
        deno[3][1] = -deno[1][3];
        deno[3][2] = -deno[1][0];
        deno[3][3] = -deno[1][1];

        // facD for derivative integrals
        let mut fac_d = zero4x4();
        fac_d[0][0] = gam * gam.conj();
        fac_d[0][1] = gam * nu.conj();
        fac_d[0][2] = -fac_d[0][0];
        fac_d[0][3] = -fac_d[0][1];
        fac_d[1][0] = fac_d[0][1].conj();
        fac_d[1][1] = nu * nu.conj();
        fac_d[1][2] = -fac_d[1][0];
        fac_d[1][3] = -fac_d[1][1];
        fac_d[2][0] = fac_d[0][2];
        fac_d[2][1] = fac_d[0][3];
        fac_d[2][2] = fac_d[0][0];
        fac_d[2][3] = fac_d[0][1];
        fac_d[3][0] = fac_d[1][2];
        fac_d[3][1] = fac_d[1][3];
        fac_d[3][2] = fac_d[1][0];
        fac_d[3][3] = fac_d[1][1];

        for ix in 0..4 {

            for jx in 0..4 {
                if deno[ix][jx] != cx(0.0) {
                    am1[ix][jx] /= -deno[ix][jx];
                    am2[ix][jx] /= -deno[ix][jx];
                    am3[ix][jx] /= -deno[ix][jx];
                } else {
                    let h_val = cx(params.h[idx]);
                    am1[ix][jx] = h_val * cx((-expos_einv_int[idx][0]).exp());
                    am2[ix][jx] = h_val * cx((-expos_einv_int[idx][1]).exp());
                    am3[ix][jx] = h_val * cx((-expos_einv_int[idx][2]).exp());
                }
            }
        }

        // Apply facD for derivative integrals
        let am4 = hadamard4(&am1, &fac_d);
        let am5 = hadamard4(&am2, &fac_d);
        let am6 = hadamard4(&am3, &fac_d);

        // Apply outer products of D columns
        let cd0 = conj4(&d_new[0]);
        let cd1 = conj4(&d_new[1]);

        aux_mtrx[idx][0] = hadamard4(&outer4(&d_new[0], &cd0), &am1);
        aux_mtrx[idx][1] = hadamard4(&outer4(&d_new[1], &cd1), &am2);
        aux_mtrx[idx][2] = hadamard4(&outer4(&d_new[1], &cd0), &am3);
        aux_mtrx[idx][3] = hadamard4(&outer4(&d_new[0], &cd0), &am4);
        aux_mtrx[idx][4] = hadamard4(&outer4(&d_new[1], &cd1), &am5);
        aux_mtrx[idx][5] = hadamard4(&outer4(&d_new[1], &cd0), &am6);
    }

    // Compute A/B ratio and eigenvector at surface
    let a_by_b = -ymat[1][2] / ymat[0][2];
    let y0: Vec4 = [
        a_by_b * ymat[0][0] + ymat[1][0],
        a_by_b * ymat[0][1] + ymat[1][1],
        a_by_b * ymat[0][2] + ymat[1][2],
        a_by_b * ymat[0][3] + ymat[1][3],
    ];
    let inv_abs_y0_2 = 1.0 / (y0[1] * y0[1].conj()).re;
    if w > 12.0 && w < 25.0 {
        // println!("R_y0={:?} {:?}", y0[0], y0[1]);
    }
    let elip = (y0[0] / (I * y0[1])).conj();
    let a_by_b_fac = cx((expo_prov2 - expo_prov1).exp());
    let _ = a_by_b_fac; // Used implicitly in v2 below

    // v2 matrix for integration coefficients
    let v2: Mat2x2 = [
        [a_by_b * a_by_b.conj(), a_by_b * cx(1.0)],
        [cx(1.0) * a_by_b.conj(), cx(1.0)],
    ];
    let exps_v2: [[Float; 2]; 2] = [
        [2.0 * (expo_prov2 - expo_prov1), expo_prov2 - expo_prov1],
        [expo_prov2 - expo_prov1, 0.0],
    ];

    // Accumulate integrals over all layers
    let mut integral = [cx(0.0); 2]; // Kinetic energy (horizontal, vertical)
    let mut integral_i1: Float = 0.0;
    let mut integral_d2: Float = 0.0;

    for idx in 0..ncapas {
        // Compute p1, p2, p3 (integration weights)
        let p1: Cx;
        let p2: Cx;
        let p3: Cx;

        if idx == 0 {
            p1 = v2[0][0] * cx((exps_v2[0][0] + expos_einv_int[idx][0] - 2.0 * expo_prov2).exp());
            p2 = v2[1][1] * cx((exps_v2[1][1] + expos_einv_int[idx][1] - 2.0 * expo_prov2).exp());
            p3 = v2[1][0] * cx((exps_v2[1][0] + expos_einv_int[idx][2] - 2.0 * expo_prov2).exp());
        } else {
            // With Qacum
            // Fortran: p1 = dot_product(Qacum(1,:), matmul(Qacum(1,:), M))
            //        = sum_n conjg(Q(1,n)) * sum_m Q(1,m) * M(m,n)
            let qa = &qacum_layers[idx];
            let eqa = expos_qacum[idx];

            p1 = {
                let mut s = cx(0.0);
                for m in 0..2 { for n in 0..2 {
                    s += qa[0][n].conj() * qa[0][m] * v2[m][n]
                        * cx((exps_v2[m][n] + expos_einv_int[idx][0] - 2.0 * expo_prov2 + 2.0 * eqa).exp());
                }}
                s
            };
            p2 = {
                let mut s = cx(0.0);
                for m in 0..2 { for n in 0..2 {
                    s += qa[1][n].conj() * qa[1][m] * v2[m][n]
                        * cx((exps_v2[m][n] + expos_einv_int[idx][1] - 2.0 * expo_prov2 + 2.0 * eqa).exp());
                }}
                s
            };
            p3 = {
                let mut s = cx(0.0);
                for m in 0..2 { for n in 0..2 {
                    s += qa[0][n].conj() * qa[1][m] * v2[m][n]
                        * cx((exps_v2[m][n] + expos_einv_int[idx][2] - 2.0 * expo_prov2 + 2.0 * eqa).exp());
                }}
                s
            };
        }
        
        if w > 12.0 && w < 25.0 {
            // println!("R_p3={:?} index={}", p3, idx);
        }

        // Kinetic energy and derivative integrals
        let mut int_parcial = [cx(0.0); 2];
        let mut int_parcial2 = [cx(0.0); 2];
        for comp in 0..2 {
            let row = &l_layers[idx][comp];
            int_parcial[comp] =
                quad4_fortran(row, &aux_mtrx[idx][0]) * p1
                + quad4_fortran(row, &aux_mtrx[idx][1]) * p2
                + cx(2.0) * cx((quad4_fortran(row, &aux_mtrx[idx][2]) * p3).re);

            int_parcial2[comp] =
                quad4_fortran(row, &aux_mtrx[idx][3]) * p1
                + quad4_fortran(row, &aux_mtrx[idx][4]) * p2
                + cx(2.0) * cx((quad4_fortran(row, &aux_mtrx[idx][5]) * p3).re);
            integral[comp] += cx(params.rho[idx]) * int_parcial[comp];

            if w > 12.0 && w < 25.0 && comp == 0 {
                // println!("R_int_parcial={:?} index={}", int_parcial[0], idx);
            }

            let alpha_beta_ratio = params.alpha[idx] * params.alpha[idx]
                / (params.beta[idx] * params.beta[idx]);

            if comp == 0 {
                integral_i1 += params.mu[idx] * (alpha_beta_ratio * int_parcial[comp].re + 0.0);
                integral_d2 += params.mu[idx] * int_parcial2[comp].re;
            } else {
                integral_i1 += params.mu[idx] * int_parcial[comp].re;
                integral_d2 += params.mu[idx] * alpha_beta_ratio * int_parcial2[comp].re;
            }
        }
    }
    // Scale by normalization
    let integral1 = (integral[0] * cx(inv_abs_y0_2)).re;
    let integral2 = (integral[1] * cx(inv_abs_y0_2)).re;
    integral_i1 *= inv_abs_y0_2;
    integral_d2 *= inv_abs_y0_2;

    if w > 12.0 && w < 25.0 {
        // println!("R_I1={:?} R_I2={:?}", integral[0], integral[1]);
    }

    WangRayleighResult {
        ellipticity: elip,
        i1: integral1,
        i2: integral2,
        ii1: integral_i1,
        id2: integral_d2,
    }
}

// =============================================================================
// Love waves — Wang method
// =============================================================================

/// Result of the Wang method for Love waves.
pub struct WangLoveResult {
    /// Energy integral.
    pub integral: Float,
    /// Ratio of mu-weighted integral to rho-weighted integral (for group velocity).
    pub integral_ratio: Float,
}

/// Implements the Wang method for Love waves.
///
/// Translates Fortran `SUBROUTINE WangMethod_L(Integral, Integralratio, W, SLOW)`.
pub fn wang_love(w: Float, slow: Float, params: &ModelParams) -> WangLoveResult {
    let ncapas = params.nlayers;

    let mut l_layers: Vec<Mat2x2> = vec![zero2x2(); ncapas];
    let mut aux_mtrx: Vec<Mat2x2> = vec![zero2x2(); ncapas];
    let mut expos_einv_int: Vec<Float> = vec![0.0; ncapas];
    let mut expos_qacum: Vec<Float> = vec![0.0; ncapas];
    let mut qacum_vals: Vec<Float> = vec![1.0; ncapas];

    let mut expo: Float = 0.0;

    // Halfspace
    let idx = ncapas - 1;
    let nu = nu_from_slow(slow, w, params.slow_s[idx]);
    l_layers[idx] = l_matrix_sh(params.mu[idx], nu);
    let d: Vec2 = [cx(1.0), cx(0.0)];
    let mut yv: Vec2 = matvec2(&l_layers[idx], &d);

    // AuxMtrx for halfspace
    aux_mtrx[idx][0][0] = cx(-1.0) / (-nu + (-nu).conj());

    // Propagate upwards
    for idx in (0..ncapas - 1).rev() {
        let nu = nu_from_slow(slow, w, params.slow_s[idx]);
        l_layers[idx] = l_matrix_sh(params.mu[idx], nu);
        let linv = l_matrix_sh_inv(params.mu[idx], nu);

        let d = matvec2(&linv, &yv);
        let q = 1.0 / dot2(&d, &d).re.sqrt();

        // Update Qacum
        for il in (idx + 1..ncapas).rev() {
            qacum_vals[il] *= q;
            expos_qacum[il] -= expo;
        }

        let d_scaled: Vec2 = [d[0] * cx(q), d[1] * cx(q)];

        let (einv1, ep) = e_matrix_norma_sh(-params.h[idx], nu);
        expo = ep;
        yv = matvec2(&matmul2(&l_layers[idx], &einv1), &d_scaled);

        // AuxMtrx
        let diag_e = diag2(&einv1);
        let mut am = outer2(&diag_e, &conj2(&diag_e));
        let sub = cx((-2.0_f64 * expo).exp());
        for i in 0..2 {
            for j in 0..2 {
                am[i][j] -= sub;
            }
        }
        expos_einv_int[idx] = 2.0 * expo;

        // Denominators for Love
        let mut deno = zero2x2();
        deno[1][0] = cx(2.0 * nu.im) * I;
        deno[1][1] = cx(2.0 * nu.re);
        deno[0][0] = -deno[1][1];
        deno[0][1] = -deno[1][0];

        for ix in 0..2 {
            for jx in 0..2 {
                if deno[ix][jx] != cx(0.0) {
                    am[ix][jx] /= -deno[ix][jx];
                } else {
                    am[ix][jx] = cx(params.h[idx]) * cx((-expos_einv_int[idx]).exp());
                }
            }
        }

        aux_mtrx[idx] = hadamard2(&outer2(&d_scaled, &conj2(&d_scaled)), &am);
    }

    // Accumulate integrals
    let inv_abs_y0_2 = 1.0 / (yv[0] * yv[0].conj()).re;
    let mut integral_rho = cx(0.0);
    let mut integral_mu = cx(0.0);

    for idx in 0..ncapas {
        let p1: Float = if idx == 0 {
            (expos_einv_int[idx] - 2.0 * expo).exp()
        } else {
            let qa = qacum_vals[idx];
            qa * (expos_einv_int[idx] - 2.0 * expo + 2.0 * expos_qacum[idx]).exp() * qa
        };

        let row = &l_layers[idx][0]; // First row of L
        let int_parcial = (row[0] * row[0] * aux_mtrx[idx][0][0]
            + row[0] * row[1] * aux_mtrx[idx][0][1]
            + row[1] * row[0] * aux_mtrx[idx][1][0]
            + row[1] * row[1] * aux_mtrx[idx][1][1])
            * cx(p1);

        integral_rho += cx(params.rho[idx]) * int_parcial;
        integral_mu += cx(params.mu[idx]) * int_parcial;
    }

    integral_rho *= cx(inv_abs_y0_2);
    integral_mu *= cx(inv_abs_y0_2);

    let ratio = (integral_mu / integral_rho).re;

    WangLoveResult {
        integral: integral_rho.re,
        integral_ratio: ratio,
    }
}

/// Element-wise multiply two 2×2 matrices.
fn hadamard2(a: &Mat2x2, b: &Mat2x2) -> Mat2x2 {
    [
        [a[0][0] * b[0][0], a[0][1] * b[0][1]],
        [a[1][0] * b[1][0], a[1][1] * b[1][1]],
    ]
}
