//! Matrix operations and propagator matrix routines.
//!
//! Translates Fortran `aux_procedures.f90`:
//!   matmul1x1, diag, det2x2, gam_val, nu_val, EmatrixNorma,
//!   Lmatrix, LmatrixInv, CmplxWLmatrix, CmplxWLmatrixInv,
//!   LmatrixSH, LmatrixInv2, EmatrixNorma2, HALFSPACE_RAYLEIGH.

use crate::hvf::types::*;

// =============================================================================
// Basic matrix/vector operations
// =============================================================================

/// Multiply two 4×4 complex matrices.
pub fn matmul4(a: &Mat4x4, b: &Mat4x4) -> Mat4x4 {
    let mut c = zero4x4();
    for i in 0..4 {
        for j in 0..4 {
            let mut s = Cx::new(0.0, 0.0);
            for k in 0..4 {
                s += a[i][k] * b[k][j];
            }
            c[i][j] = s;
        }
    }
    c
}

/// Multiply two 2×2 complex matrices.
pub fn matmul2(a: &Mat2x2, b: &Mat2x2) -> Mat2x2 {
    let mut c = zero2x2();
    for i in 0..2 {
        for j in 0..2 {
            c[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j];
        }
    }
    c
}

/// Multiply a 4×4 matrix by a 4-vector.
pub fn matvec4(a: &Mat4x4, v: &Vec4) -> Vec4 {
    let mut r = [Cx::new(0.0, 0.0); 4];
    for i in 0..4 {
        r[i] = a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2] + a[i][3] * v[3];
    }
    r
}

pub fn vecmat4(v: &Vec4, a: &Mat4x4) -> Vec4 {
    let mut r = [Cx::new(0.0, 0.0); 4];
    for j in 0..4 {
        r[j] = v[0] * a[0][j] + v[1] * a[1][j] + v[2] * a[2][j] + v[3] * a[3][j];
    }
    r
}

/// Multiply a 2×2 matrix by a 2-vector.
pub fn matvec2(a: &Mat2x2, v: &Vec2) -> Vec2 {
    [a[0][0] * v[0] + a[0][1] * v[1], a[1][0] * v[0] + a[1][1] * v[1]]
}

pub fn vecmat2(v: &Vec2, a: &Mat2x2) -> Vec2 {
    [v[0] * a[0][0] + v[1] * a[1][0], v[0] * a[0][1] + v[1] * a[1][1]]
}

/// Multiply a 4×4 matrix by a 4×2 "tall" matrix (stored as [Vec4; 2] columns).
pub fn matmul4x2(a: &Mat4x4, b: &[Vec4; 2]) -> [Vec4; 2] {
    [matvec4(a, &b[0]), matvec4(a, &b[1])]
}

/// Outer product of two complex vectors: A(n×1) × B(1×m) → matrix(n×m).
/// Replaces Fortran `matmul1x1(A,B)`.
pub fn outer4(a: &Vec4, b: &Vec4) -> Mat4x4 {
    let mut m = zero4x4();
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] = a[i] * b[j];
        }
    }
    m
}

/// Outer product for 2-vectors.
pub fn outer2(a: &Vec2, b: &Vec2) -> Mat2x2 {
    [[a[0] * b[0], a[0] * b[1]], [a[1] * b[0], a[1] * b[1]]]
}

/// Extract diagonal of a 4×4 matrix as a vector.
pub fn diag4(a: &Mat4x4) -> Vec4 {
    [a[0][0], a[1][1], a[2][2], a[3][3]]
}

/// Extract diagonal of a 2×2 matrix as a vector.
pub fn diag2(a: &Mat2x2) -> Vec2 {
    [a[0][0], a[1][1]]
}

/// Determinant of a 2×2 complex matrix.
pub fn det2x2(a: &Mat2x2) -> Cx {
    a[0][0] * a[1][1] - a[1][0] * a[0][1]
}

/// Dot product of two complex 4-vectors: sum(a[i] * b[i]).
pub fn dot4(a: &Vec4, b: &Vec4) -> Cx {
    a[0].conj() * b[0] + a[1].conj() * b[1] + a[2].conj() * b[2] + a[3].conj() * b[3]
}

/// Dot product of two complex 2-vectors (conjugates the first vector, like Fortran).
pub fn dot2(a: &Vec2, b: &Vec2) -> Cx {
    a[0].conj() * b[0] + a[1].conj() * b[1]
}

/// Conjugate of a 4-vector.
pub fn conj4(v: &Vec4) -> Vec4 {
    [v[0].conj(), v[1].conj(), v[2].conj(), v[3].conj()]
}

/// Conjugate of a 2-vector.
pub fn conj2(v: &Vec2) -> Vec2 {
    [v[0].conj(), v[1].conj()]
}

/// Element-wise multiply two 4×4 matrices.
pub fn hadamard4(a: &Mat4x4, b: &Mat4x4) -> Mat4x4 {
    let mut c = zero4x4();
    for i in 0..4 {
        for j in 0..4 {
            c[i][j] = a[i][j] * b[i][j];
        }
    }
    c
}

/// Multiply 4×4 matrix by a scalar.
pub fn scale4(a: &Mat4x4, s: Cx) -> Mat4x4 {
    let mut c = zero4x4();
    for i in 0..4 {
        for j in 0..4 {
            c[i][j] = a[i][j] * s;
        }
    }
    c
}

/// Add two 4×4 matrices.
pub fn add4(a: &Mat4x4, b: &Mat4x4) -> Mat4x4 {
    let mut c = zero4x4();
    for i in 0..4 {
        for j in 0..4 {
            c[i][j] = a[i][j] + b[i][j];
        }
    }
    c
}

/// v^T * M * v  (quadratic form with 4-vector and 4×4 matrix).
pub fn quad4(v: &Vec4, m: &Mat4x4) -> Cx {
    dot4(v, &matvec4(m, v))
}

// =============================================================================
// Wavenumber functions
// =============================================================================

/// Vertical wavenumber for P-waves: γ = -i*ω/α * sqrt(1 - α²/c²)
/// Replaces Fortran `gam_val(c, w, alfa)`.
pub fn gamma_val(c: Float, w: Float, alpha: Float) -> Cx {
    -I * cx(w / alpha) * (cx(1.0 - alpha * alpha / (c * c))).sqrt()
}

/// Vertical wavenumber for P-waves from slowness.
/// Replaces Fortran `gam_val_from_slow(slow, w, slowP)`.
pub fn gamma_from_slow(slow: Float, w: Float, slow_p: Float) -> Cx {
    if slow != slow_p {
        -I * cx(w * slow_p) * Cx::new(1.0 - slow * slow / (slow_p * slow_p), 0.0).sqrt()
    } else {
        -I * cx(w * slow_p) * Cx::new(-Float::MIN_POSITIVE, 0.0).sqrt()
    }
}

/// Vertical wavenumber for S-waves: ν = -i*ω/β * sqrt(1 - β²/c²)
/// Replaces Fortran `nu_val(c, w, bta)`.
pub fn nu_val(c: Float, w: Float, beta: Float) -> Cx {
    -I * cx(w / beta) * (cx(1.0 - beta * beta / (c * c))).sqrt()
}

/// Vertical wavenumber for S-waves from slowness.
/// Replaces Fortran `nu_val_from_slow(slow, w, slowS)`.
pub fn nu_from_slow(slow: Float, w: Float, slow_s: Float) -> Cx {
    if slow != slow_s {
        -I * cx(w * slow_s) * Cx::new(1.0 - slow * slow / (slow_s * slow_s), 0.0).sqrt()
    } else {
        -I * cx(w * slow_s) * Cx::new(-Float::MIN_POSITIVE, 0.0).sqrt()
    }
}

// =============================================================================
// Propagator E-matrix (normalized exponential layer matrix)
// =============================================================================

/// Computes the normalized exponential propagator matrix for PSV (4×4).
///
/// `norm_type`: 1 = normalize by gamma, 2 = normalize by nu, 3 (or None) = fastest diverging.
/// Returns (matrix, expo) where expo is the normalization exponent.
///
/// Replaces Fortran `EmatrixNorma(salida, expo, z, gam, nu, tipo_norma)`.
pub fn e_matrix_norma(z: Float, gam: Cx, nu: Cx, norm_type: Option<u8>) -> (Mat4x4, Float) {
    let mut m = zero4x4();
    let rg = gam.re * z;
    let ig = gam.im * z;
    let rnu = nu.re * z;
    let inu = nu.im * z;
    let expo: Float;

    let nt = norm_type.unwrap_or(3);

    if nt == 3 {
        if rg > 0.0 {
            if rg > rnu {
                let aux1 = (I * cx(inu)).exp();
                m[2][2] = (I * cx(ig)).exp();
                m[0][0] = m[2][2].conj() * cx((-2.0_f64 * rg).exp());
                m[1][1] = aux1.conj() * cx((-1.0_f64 * rnu - rg).exp());
                m[3][3] = aux1 * cx((1.0_f64 * rnu - rg).exp());
                expo = rg;
            } else {
                let aux1 = (I * cx(ig)).exp();
                m[3][3] = (I * cx(inu)).exp();
                m[0][0] = aux1.conj() * cx((-1.0_f64 * rg - rnu).exp());
                m[1][1] = m[3][3].conj() * cx((-2.0_f64 * rnu).exp());
                m[2][2] = aux1 * cx((1.0_f64 * rg - rnu).exp());
                expo = rnu;
            }
        } else {
            if rg < rnu {
                let aux1 = (I * cx(inu)).exp();
                m[0][0] = (-I * cx(ig)).exp();
                m[1][1] = aux1.conj() * cx((-1.0_f64 * rnu + rg).exp());
                m[2][2] = m[0][0].conj() * cx((2.0_f64 * rg).exp());
                m[3][3] = aux1 * cx((1.0_f64 * rnu + rg).exp());
                expo = -rg;
            } else {
                let aux1 = (I * cx(ig)).exp();
                m[0][0] = aux1.conj() * cx((-rg + rnu).exp());
                m[1][1] = (-I * cx(inu)).exp();
                m[2][2] = aux1 * cx((rg + rnu).exp());
                m[3][3] = m[1][1].conj() * cx((2.0_f64 * rnu).exp());
                expo = -rnu;
            }
        }
    } else if rg > 0.0 {
        if nt == 1 {
            let aux1 = (I * cx(inu)).exp();
            m[2][2] = (I * cx(ig)).exp();
            m[0][0] = m[2][2].conj() * cx((-2.0_f64 * rg).exp());
            m[1][1] = aux1.conj() * cx((-1.0_f64 * rnu - rg).exp());
            m[3][3] = aux1 * cx((1.0_f64 * rnu - rg).exp());
            expo = rg;
        } else {
            // nt == 2
            let aux1 = (I * cx(ig)).exp();
            m[0][0] = aux1.conj() * cx((-1.0_f64 * rg - rnu).exp());
            m[3][3] = (I * cx(inu)).exp();
            m[1][1] = m[3][3].conj() * cx((-2.0_f64 * rnu).exp());
            m[2][2] = aux1 * cx((1.0_f64 * rg - rnu).exp());
            expo = rnu;
        }
    } else {
        if nt == 1 {
            let aux1 = (I * cx(inu)).exp();
            m[0][0] = (-I * cx(ig)).exp();
            m[1][1] = aux1.conj() * cx((-1.0_f64 * rnu + rg).exp());
            m[2][2] = m[0][0].conj() * cx((2.0_f64 * rg).exp());
            m[3][3] = aux1 * cx((1.0_f64 * rnu + rg).exp());
            expo = -rg;
        } else {
            // nt == 2
            let aux1 = (I * cx(ig)).exp();
            m[0][0] = aux1.conj() * cx((-rg + rnu).exp());
            m[1][1] = (-I * cx(inu)).exp();
            m[2][2] = aux1 * cx((rg + rnu).exp());
            m[3][3] = m[1][1].conj() * cx((2.0_f64 * rnu).exp());
            expo = -rnu;
        }
    }

    (m, expo)
}

/// Computes the normalized exponential propagator matrix for SH/Love (2×2).
///
/// Replaces Fortran `EmatrixNorma2(salida, expo, z, nu)`.
pub fn e_matrix_norma_sh(z: Float, nu: Cx) -> (Mat2x2, Float) {
    let mut m = zero2x2();
    let rnu = nu.re * z;
    let inu = nu.im * z;
    let expo: Float;

    if rnu > 0.0 {
        m[1][1] = (I * cx(inu)).exp();
        m[0][0] = m[1][1].conj() * cx((-2.0_f64 * rnu).exp());
        expo = rnu;
    } else {
        m[0][0] = (-I * cx(inu)).exp();
        m[1][1] = m[0][0].conj() * cx((2.0_f64 * rnu).exp());
        expo = -rnu;
    }

    (m, expo)
}

// =============================================================================
// L-matrix and L-inverse for PSV (Rayleigh) — real ω, c
// =============================================================================

/// Computes the L-matrix for PSV wave propagation (real ω, c).
///
/// Replaces Fortran `Lmatrix(salida, alfa, bta, mu, w, c, gam, nu)`.
pub fn l_matrix(alpha: Float, beta: Float, mu: Float, w: Float, c: Float, gam: Cx, nu: Cx) -> Mat4x4 {
    let mut m = zero4x4();
    let k = w / c;
    let aux1 = cx(mu) * (cx(k * k) + nu * nu) / cx(w);
    let aux2 = cx(2.0 * mu * k);

    m[0][0] = cx(alpha * k / w);
    m[0][1] = cx(beta) * nu / cx(w);
    m[0][2] = m[0][0];
    m[0][3] = m[0][1];

    m[1][0] = cx(alpha) * gam / cx(w);
    m[1][1] = cx(beta * k / w);
    m[1][2] = -m[1][0];
    m[1][3] = -m[1][1];

    m[2][2] = aux2 * m[1][0]; // 2*alfa*mu*k*gam/w
    m[2][0] = -m[2][2];
    m[2][3] = cx(beta) * aux1;
    m[2][1] = -m[2][3];

    m[3][0] = -cx(alpha) * aux1;
    m[3][1] = -aux2 * m[0][1];
    m[3][2] = m[3][0];
    m[3][3] = m[3][1]; // -2*bta*mu*k*nu/w

    m
}

/// Computes the L-inverse matrix for PSV (real ω, c).
///
/// Replaces Fortran `LmatrixInv(salida, alfa, bta, mu, w, c, gam, nu)`.
pub fn l_matrix_inv(alpha: Float, beta: Float, mu: Float, w: Float, c: Float, gam: Cx, nu: Cx) -> Mat4x4 {
    let mut m = zero4x4();
    let k = w / c;
    let aux1 = cx(0.5 * beta) * (cx(k * k) + nu * nu);
    let aux2 = beta * beta / alpha;
    let aux3 = cx(1.0 / w);

    m[0][0] = cx(aux2 * k);
    m[2][1] = cx(beta) * aux1 / (cx(alpha) * gam);
    m[0][1] = -m[2][1];
    m[0][3] = cx(0.5 * aux2 / mu);
    m[0][2] = -m[0][3] * (cx(k) / gam);

    m[1][0] = -aux1 / nu;
    m[1][1] = cx(beta * k);
    m[1][2] = cx(0.5 * beta / mu);
    m[1][3] = -m[1][2] * (cx(k) / nu);

    m[2][0] = m[0][0];
    m[2][2] = cx(0.5) * m[0][0] / (gam * cx(mu));
    m[2][3] = m[0][3];

    m[3][0] = m[1][0];
    m[3][1] = -m[1][1]; // -bta*k
    m[3][2] = -m[1][2];
    m[3][3] = m[1][3];

    // Scale entire matrix by 1/w
    for i in 0..4 {
        for j in 0..4 {
            m[i][j] *= aux3;
        }
    }
    m
}

// =============================================================================
// L-matrix and L-inverse for PSV — complex ω, c (for body waves)
// =============================================================================

/// Computes the L-matrix for PSV wave propagation (complex ω, c).
///
/// Replaces Fortran `CmplxWLmatrix(salida, alfa, bta, mu, w, c, gam, nu)`.
pub fn l_matrix_complex(alpha: Float, beta: Float, mu: Float, w: Cx, c: Cx, gam: Cx, nu: Cx) -> Mat4x4 {
    let mut m = zero4x4();
    let k = w.re / c.re;
    let aux1 = cx(mu) * (cx(k * k) + nu * nu) / w;
    let aux2 = cx(2.0 * mu * k);

    m[0][0] = cx(alpha * k) / w;
    m[0][1] = cx(beta) * nu / w;
    m[0][2] = m[0][0];
    m[0][3] = m[0][1];

    m[1][0] = cx(alpha) * gam / w;
    m[1][1] = cx(beta * k) / w;
    m[1][2] = -m[1][0];
    m[1][3] = -m[1][1];

    m[2][2] = aux2 * m[1][0];
    m[2][0] = -m[2][2];
    m[2][3] = cx(beta) * aux1;
    m[2][1] = -m[2][3];

    m[3][0] = -cx(alpha) * aux1;
    m[3][1] = -aux2 * m[0][1];
    m[3][2] = m[3][0];
    m[3][3] = m[3][1];

    m
}

/// Computes the L-inverse matrix for PSV (complex ω, c).
///
/// Replaces Fortran `CmplxWLmatrixInv(salida, alfa, bta, mu, w, c, gam, nu)`.
pub fn l_matrix_inv_complex(alpha: Float, beta: Float, mu: Float, w: Cx, c: Cx, gam: Cx, nu: Cx) -> Mat4x4 {
    let mut m = zero4x4();
    let k = w.re / c.re;
    let aux1 = cx(0.5 * beta) * (cx(k * k) + nu * nu);
    let aux2 = beta * beta / alpha;
    let aux3 = Cx::new(1.0, 0.0) / w;

    m[0][0] = cx(aux2 * k);
    m[2][1] = cx(beta) * aux1 / (cx(alpha) * gam);
    m[0][1] = -m[2][1];
    m[0][3] = cx(0.5 * aux2 / mu);
    m[0][2] = -m[0][3] * (cx(k) / gam);

    m[1][0] = -aux1 / nu;
    m[1][1] = cx(beta * k);
    m[1][2] = cx(0.5 * beta / mu);
    m[1][3] = -m[1][2] * (cx(k) / nu);

    m[2][0] = m[0][0];
    m[2][2] = cx(0.5) * m[0][0] / (gam * cx(mu));
    m[2][3] = m[0][3];

    m[3][0] = m[1][0];
    m[3][1] = -m[1][1];
    m[3][2] = -m[1][2];
    m[3][3] = m[1][3];

    for i in 0..4 {
        for j in 0..4 {
            m[i][j] *= aux3;
        }
    }
    m
}

// =============================================================================
// L-matrix and L-inverse for SH (Love waves) — 2×2
// =============================================================================

/// Computes the L-matrix for SH wave propagation (2×2).
///
/// Replaces Fortran `LmatrixSH(salida, mu, nu)`.
pub fn l_matrix_sh(mu: Float, nu: Cx) -> Mat2x2 {
    let one = Cx::new(1.0, 0.0);
    let numu = nu * cx(mu);
    [[one, one], [-numu, numu]]
}

/// Computes the L-inverse matrix for SH (2×2).
///
/// Replaces Fortran `LmatrixInv2(salida, mu, nu)`.
pub fn l_matrix_sh_inv(mu: Float, nu: Cx) -> Mat2x2 {
    let half = Cx::new(0.5, 0.0);
    let inv_2numu = Cx::new(1.0, 0.0) / (cx(2.0) * nu * cx(mu));
    [[half, -inv_2numu], [half, inv_2numu]]
}

// =============================================================================
// Halfspace Rayleigh velocity (Newton-Raphson)
// =============================================================================

/// Computes the Rayleigh wave velocity for a halfspace using Newton-Raphson.
///
/// Replaces Fortran `HALFSPACE_RAYLEIGH(iLayer)`.
/// Returns the Rayleigh slowness of the halfspace.
pub fn halfspace_rayleigh(slow_p: Float, slow_s: Float) -> Float {
    let boa = slow_p / slow_s;
    let a4 = 16.0 * boa * boa - 16.0;
    let a3 = 8.0 - a4;

    let mut x: Float = 0.9025; // Initial guess: (c/beta)^2 = 0.95^2
    let mut sqx = x * x;
    let mut y = x * sqx - 8.0 * sqx + a3 * x + a4;
    let mut dy: f64 = 3.0 * sqx - 16.0 * x + a3;
    x -= y / dy;

    let mut iter_nr = 0;
    while y.abs() > 1e-10 && iter_nr < 100 {
        iter_nr += 1;
        sqx = x * x;
        y = x * sqx - 8.0 * sqx + a3 * x + a4;
        dy = 3.0 * sqx - 16.0 * x + a3;
        
        if dy.abs() < 1e-12 {
            break; // Avoid division by zero
        }
        x -= y / dy;
    }

    slow_s / x.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nan() {
        let slow = 0.001;
        let c = 1.0 / slow;
        let w = 6.28;
        let alpha = 1558.0;
        let gam = gamma_val(c, w, alpha);
        println!("gam={}", gam);
    }
}

/// Computes dot_product(v, matmul(v, m)) as in Fortran
pub fn quad4_fortran(v: &Vec4, m: &Mat4x4) -> Cx {
    let mut u: Vec4 = [cx(0.0); 4];
    for j in 0..4 {
        for i in 0..4 {
            u[j] += v[i] * m[i][j];
        }
    }
    dot4(v, &u)
}

/// Computes dot_product(v, matmul(v, m)) as in Fortran
pub fn quad2_fortran(v: &Vec2, m: &Mat2x2) -> Cx {
    let mut u: Vec2 = [cx(0.0); 2];
    for j in 0..2 {
        for i in 0..2 {
            u[j] += v[i] * m[i][j];
        }
    }
    dot2(v, &u)
}
