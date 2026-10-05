//! Frequency equation whose roots define the dispersion curves.
//!
//! Translates Fortran `y.f90`: functions `Y`, `yR` (Rayleigh), `yL` (Love).

use crate::hvf::matrix::*;
use crate::hvf::model::ModelParams;
use crate::hvf::types::*;

/// Dispatcher: evaluates either Rayleigh or Love frequency equation.
///
/// Replaces Fortran `FUNCTION Y(SLOW)` which reads global `ISRAYLEIGH`.
pub fn y_dispatch(slow: Float, omega: Float, params: &ModelParams, is_rayleigh: bool) -> Float {
    if is_rayleigh {
        y_rayleigh(slow, omega, params)
    } else {
        y_love(slow, omega, params)
    }
}

/// Rayleigh-wave frequency equation.
///
/// Returns a real value whose sign change indicates a dispersion curve root.
/// Uses the propagator matrix method with normalization (Wang, 1999).
///
/// Replaces Fortran `FUNCTION yR(slow)`.
pub fn y_rayleigh(slow: Float, omega: Float, params: &ModelParams) -> Float {
    let w = omega;
    let c = 1.0 / slow;
    let ncapas = params.nlayers;

    // Initialize Qacum as 2×2 identity
    let mut qacum = eye2x2();

    // Start from the halfspace (bottom layer)
    let idx = ncapas - 1; // 0-indexed
    let gam = gamma_val(c, w, params.alpha[ncapas - 1]);
    let nu = nu_val(c, w, params.beta[ncapas - 1]);
    let l = l_matrix(params.alpha[idx], params.beta[idx], params.mu[idx], w, c, gam, nu);

    // D = [[1,0],[0,1],[0,0],[0,0]]  (4×2, columns are the two basis vectors)
    let mut yb: [Vec4; 2] = [
        [cx(1.0), cx(0.0), cx(0.0), cx(0.0)],
        [cx(0.0), cx(1.0), cx(0.0), cx(0.0)],
    ];
    yb = matmul4x2(&l, &yb);

    // Propagate upwards through layers
    for idx in (0..ncapas - 1).rev() {
        let gam = gamma_val(c, w, params.alpha[idx]);
        let nu = nu_val(c, w, params.beta[idx]);
        let l = l_matrix(params.alpha[idx], params.beta[idx], params.mu[idx], w, c, gam, nu);
        let r_inv = l_matrix_inv(params.alpha[idx], params.beta[idx], params.mu[idx], w, c, gam, nu);

        let d: [Vec4; 2] = matmul4x2(&r_inv, &yb);

        // Normalize: Q is a 2×2 matrix from the first two rows of D
        let norm = 1.0
            / (dot4(&d[0], &d[0]).re * dot4(&d[1], &d[1]).re).sqrt();

        // Q = [[D(2,2), -D(1,2)], [-D(2,1), D(1,1)]] * norma
        let q: Mat2x2 = [
            [d[1][1] * cx(norm), -d[1][0] * cx(norm)],
            [-d[0][1] * cx(norm), d[0][0] * cx(norm)],
        ];

        // D = D * Q  (but we only need the diagonal entries for propagation)
        let d_new: [Vec4; 2] = [
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
        // Zero out off-diagonal: D(1,2)=0, D(2,1)=0 in Fortran indexing
        let mut d_clean = d_new;
        d_clean[1][0] = cx(0.0); // D(1,2) in 0-indexed col=1, row=0
        d_clean[0][1] = cx(0.0); // D(2,1) in 0-indexed col=0, row=1

        qacum = matmul2(&qacum, &q);

        // Einv1: normalize by gamma type (tipo_norma=1)
        let (einv1, _expo1) = e_matrix_norma(-params.h[idx], gam, nu, Some(1));
        let col0 = matvec4(&matmul4(&l, &einv1), &d_clean[0]);

        // Einv2: normalize by nu type (tipo_norma=2), with Einv2(1,1)=0
        let (mut einv2, _expo2) = e_matrix_norma(-params.h[idx], gam, nu, Some(2));
        einv2[0][0] = cx(0.0);
        let col1 = matvec4(&matmul4(&l, &einv2), &d_clean[1]);

        yb = [col0, col1];
    }

    // Result: det of the 2×2 submatrix formed by rows 3,4 of Yb * conj(Qacum)
    // Fortran: yR = real(det2x2(matmul(Yb(3:4,:), conjg(Qacum))))
    let qc: Mat2x2 = [
        [qacum[0][0].conj(), qacum[0][1].conj()],
        [qacum[1][0].conj(), qacum[1][1].conj()],
    ];
    // Yb(3:4, :) in Fortran = rows 2,3 in 0-indexed, columns are our [Vec4; 2]
    let sub: Mat2x2 = [
        [
            yb[0][2] * qc[0][0] + yb[1][2] * qc[1][0],
            yb[0][2] * qc[0][1] + yb[1][2] * qc[1][1],
        ],
        [
            yb[0][3] * qc[0][0] + yb[1][3] * qc[1][0],
            yb[0][3] * qc[0][1] + yb[1][3] * qc[1][1],
        ],
    ];
    det2x2(&sub).re
}

/// Love-wave frequency equation.
///
/// Returns a real value whose sign change indicates a dispersion curve root.
///
/// Replaces Fortran `FUNCTION yL(slow)`.
pub fn y_love(slow: Float, omega: Float, params: &ModelParams) -> Float {
    let w = omega;
    let c = 1.0 / slow;
    let ncapas = params.nlayers;

    // Start from the halfspace
    let idx = ncapas - 1;
    let nu = nu_val(c, w, params.beta[idx]);
    let l = l_matrix_sh(params.mu[idx], nu);

    // D = [1, 0]
    let d: Vec2 = [cx(1.0), cx(0.0)];
    let mut yb: Vec2 = matvec2(&l, &d);

    // Propagate upwards
    for idx in (0..ncapas - 1).rev() {
        let nu = nu_val(c, w, params.beta[idx]);
        let l = l_matrix_sh(params.mu[idx], nu);
        let linv = l_matrix_sh_inv(params.mu[idx], nu);

        let d = matvec2(&linv, &yb);

        // Normalize
        let q = 1.0 / (d[0] * d[0].conj() + d[1] * d[1].conj()).re.sqrt();
        let d_scaled: Vec2 = [d[0] * cx(q), d[1] * cx(q)];

        let (einv1, _expo) = e_matrix_norma_sh(-params.h[idx], nu);
        yb = matvec2(&matmul2(&l, &einv1), &d_scaled);
    }

    // Result: real part of Yb(2)
    yb[1].re
}
