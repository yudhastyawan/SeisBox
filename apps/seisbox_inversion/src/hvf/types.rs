//! Core type aliases and constants for HV-DFA computation.
//!
//! Maps Fortran `TYPES`, `Globales` modules to Rust.

use num_complex::Complex64;

// =============================================================================
// Type Aliases
// =============================================================================

/// Double-precision float (replaces Fortran `REAL(long_float)`).
pub type Float = f64;

/// Double-precision complex (replaces Fortran `COMPLEX(long_cmplx)`).
pub type Cx = Complex64;

// =============================================================================
// Fixed-size matrix/vector types (all computations use 2×2 or 4×4)
// =============================================================================

/// 4×4 complex matrix — used for PSV (Rayleigh) propagator matrices.
pub type Mat4x4 = [[Cx; 4]; 4];

/// 2×2 complex matrix — used for SH (Love) propagator matrices.
pub type Mat2x2 = [[Cx; 2]; 2];

/// 4-component complex vector.
pub type Vec4 = [Cx; 4];

/// 2-component complex vector.
pub type Vec2 = [Cx; 2];

// =============================================================================
// Constants
// =============================================================================

/// Imaginary unit `i`.
pub const I: Cx = Cx::new(0.0, 1.0);

/// π (double precision).
pub const PI: Float = std::f64::consts::PI;

/// 2π.
pub const TWO_PI: Float = 2.0 * PI;

// =============================================================================
// Matrix constructors
// =============================================================================

/// Create a 4×4 zero matrix.
#[inline]
pub fn zero4x4() -> Mat4x4 {
    [[Cx::new(0.0, 0.0); 4]; 4]
}

/// Create a 2×2 zero matrix.
#[inline]
pub fn zero2x2() -> Mat2x2 {
    [[Cx::new(0.0, 0.0); 2]; 2]
}

/// Create a 4×4 identity matrix.
#[inline]
pub fn eye4x4() -> Mat4x4 {
    let mut m = zero4x4();
    for i in 0..4 {
        m[i][i] = Cx::new(1.0, 0.0);
    }
    m
}

/// Create a 2×2 identity matrix.
#[inline]
pub fn eye2x2() -> Mat2x2 {
    let mut m = zero2x2();
    for i in 0..2 {
        m[i][i] = Cx::new(1.0, 0.0);
    }
    m
}

/// Convenience: create a complex number from a real value.
#[inline]
pub fn cx(re: Float) -> Cx {
    Cx::new(re, 0.0)
}

/// Convenience: create a complex number from real and imaginary parts.
#[inline]
pub fn cxri(re: Float, im: Float) -> Cx {
    Cx::new(re, im)
}
