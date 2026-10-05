//! Root-finding algorithms for dispersion curve computation.
//!
//! Translates Fortran `RootSolverTemplate.f90`: halving, Neville polynomial
//! interpolation, and search routines adapted from Geopsy.

use crate::hvf::types::Float;

/// Maximum number of iterations for root refinement.
pub const MAX_ITERATIONS: usize = 500;

/// State of a root solver, encapsulating all the global variables from the
/// Fortran `Marc` module that the solver used (G_X1, G_X2, G_Y1, G_Y2, etc.).
#[derive(Debug, Clone)]
pub struct RootSolver {
    pub x1: Float,
    pub y1: Float,
    pub x2: Float,
    pub y2: Float,
    pub polarity: Float,
    pub precision: Float,
    pub dx: Float,
    pub dx_is_absolute: bool,
}

impl RootSolver {
    pub fn new(precision: Float) -> Self {
        RootSolver {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
            polarity: 0.0,
            precision,
            dx: 0.1,
            dx_is_absolute: false,
        }
    }

    // =========================================================================
    // Helper: sign-change detection
    // =========================================================================

    /// True if range (y1, y2) contains zero (sign change).
    #[inline]
    fn is_found(y1: Float, y2: Float) -> bool {
        (y1 < 0.0 && y2 > 0.0) || (y1 > 0.0 && y2 < 0.0)
    }

    // =========================================================================
    // Iterator helpers
    // =========================================================================

    #[inline]
    fn it_next(x: Float, dx: Float, is_absolute: bool, go_down: bool) -> Float {
        if is_absolute {
            if go_down { x - dx } else { x + dx }
        } else {
            x * dx
        }
    }

    #[inline]
    fn it_at_end1(x: Float, toward: Float, go_down: bool) -> bool {
        if go_down { x <= toward } else { x >= toward }
    }

    #[inline]
    fn it_at_end2(x: Float, minimum: Float, maximum: Float, go_down: bool) -> bool {
        if go_down { x <= minimum } else { x >= maximum }
    }

    #[inline]
    fn it_begin(minimum: Float, maximum: Float, go_down: bool) -> Float {
        if go_down { maximum } else { minimum }
    }

    #[inline]
    fn it_end(minimum: Float, maximum: Float, go_down: bool) -> Float {
        if go_down { minimum } else { maximum }
    }

    // =========================================================================
    // Bisection refinement
    // =========================================================================

    /// Refines a bracketed root using bisection.
    ///
    /// Replaces Fortran `SUBROUTINE halving()`.
    pub fn halving(&mut self, y_fn: &impl Fn(Float) -> Float) {
        let mut x3: f64 = 0.5 * (self.x1 + self.x2);
        let mut y3 = y_fn(x3);

        let tmp_abs = (self.x1 + self.x2).abs();
        let max_diff = tmp_abs * self.precision;
        let min_diff = -max_diff;

        for _ in 1..MAX_ITERATIONS {
            if y3 == 0.0 {
                x3 -= (self.x2 - self.x1) * 0.1;
                y3 = y_fn(x3);
            }
            if Self::is_found(self.y1, y3) {
                self.x2 = x3;
                self.y2 = y3;
            } else {
                self.x1 = x3;
                self.y1 = y3;
            }
            let diff = self.x1 - self.x2;
            if min_diff <= diff && diff <= max_diff {
                return;
            }
            x3 = 0.5 * (self.x1 + self.x2);
            y3 = y_fn(x3);
        }
        // Maximum iteration reached — just return the best bracket we have
    }

    // =========================================================================
    // Neville polynomial interpolation + bisection
    // =========================================================================

    /// Refines a bracketed root using Neville polynomial interpolation.
    ///
    /// Falls back to bisection when the function is strongly non-linear.
    /// Replaces Fortran `SUBROUTINE neville()`.
    pub fn neville(&mut self, y_fn: &impl Fn(Float) -> Float) {
        let mut m: usize = 0;
        let mut p = [0.0_f64; 20];
        let mut nev_y = [0.0_f64; 20];

        // First halving
        let mut x3: f64 = 0.5 * (self.x1 + self.x2);
        let mut y3 = y_fn(x3);

        // Relative convergence criteria
        let max_diff = x3.abs() * self.precision;

        for _ in 1..MAX_ITERATIONS {
            if y3 == 0.0 {
                // Move x3 towards the furthest limit
                if (self.x1 - x3).abs() > (self.x2 - x3).abs() {
                    x3 = 0.5 * (self.x1 + x3);
                } else {
                    x3 = 0.5 * (self.x2 + x3);
                }
                y3 = y_fn(x3);
            }

            if Self::is_found(self.y1, y3) {
                self.x2 = x3;
                self.y2 = y3;
            } else {
                self.x1 = x3;
                self.y1 = y3;
            }

            // Convergence check
            if (self.x1 - self.x2).abs() <= max_diff {
                return;
            }

            // Check slopes: if strongly non-linear, use halving
            if Self::is_found(self.y1 - y3, y3 - self.y2) {
                x3 = 0.5 * (self.x1 + self.x2);
                y3 = y_fn(x3);
                m = 0;
            } else {
                if m > 0 {
                    p[m + 1] = x3;
                    nev_y[m + 1] = y3;
                } else {
                    p[0] = self.x1;
                    nev_y[0] = self.y1;
                    p[1] = self.x2;
                    nev_y[1] = self.y2;
                }

                // Neville iteration
                for j in (0..=m).rev() {
                    let denom = nev_y[m + 1] - nev_y[j];
                    let tmp_abs2 = denom.abs();
                    let num = nev_y[m + 1] * p[j] - nev_y[j] * p[j + 1];

                    // Check for possible overflow
                    if tmp_abs2 > 0.0 {
                        let log_num = num.abs().log2();
                        let log_den = tmp_abs2.log2();
                        if log_num - log_den < (Float::MAX_EXP as Float - 1.0) {
                            p[j] = num / denom;
                        } else {
                            p[0] = 0.5 * (self.x1 + self.x2);
                            m = 0;
                            break;
                        }
                    } else {
                        p[0] = 0.5 * (self.x1 + self.x2);
                        m = 0;
                        break;
                    }
                }
                x3 = p[0];

                // Nudge towards the limit with higher |y|
                if self.y1.abs() > self.y2.abs() {
                    x3 -= 0.1 * (x3 - self.x1);
                } else {
                    x3 -= 0.1 * (x3 - self.x2);
                }

                m += 1;
                if m > 18 {
                    m = 1;
                }

                // Ensure estimate is inside brackets
                if self.x1 <= self.x2 {
                    if x3 < self.x1 || x3 > self.x2 {
                        x3 = 0.5 * (self.x1 + self.x2);
                        m = 1;
                    }
                } else {
                    if x3 < self.x2 || x3 > self.x1 {
                        x3 = 0.5 * (self.x1 + self.x2);
                        m = 1;
                    }
                }
                y3 = y_fn(x3);
            }
        }
    }

    // =========================================================================
    // Search routines
    // =========================================================================

    /// Search for a root in the downward direction (from high to low slowness).
    ///
    /// Replaces Fortran `SEARCHDOWN(FROM, MINIMUM, MAXIMUM)`.
    pub fn search_down(
        &mut self,
        from: Float,
        minimum: Float,
        maximum: Float,
        y_fn: &impl Fn(Float) -> Float,
    ) -> bool {
        let go_down = true;
        let dx = if self.dx_is_absolute {
            self.dx
        } else {
            1.0 - self.dx
        };
        self.search2(from, minimum, maximum, dx, go_down, y_fn)
    }

    /// Search for a root in the upward direction (from low to high slowness).
    /// Returns false if `from >= toward`.
    ///
    /// Replaces Fortran `SEARCHUP(FROM, TOWARD)`.
    pub fn search_up(
        &mut self,
        from: Float,
        toward: Float,
        y_fn: &impl Fn(Float) -> Float,
    ) -> bool {
        let go_down = false;
        if from > toward {
            return false;
        }
        let dx = if self.dx_is_absolute {
            self.dx
        } else {
            1.0 + self.dx
        };
        self.search1(from, toward, dx, go_down, y_fn)
    }

    /// Directional search towards a single target bound.
    ///
    /// Replaces Fortran `SEARCH1(FROM, TOWARD, DX)`.
    fn search1(
        &mut self,
        from: Float,
        toward: Float,
        dx: Float,
        go_down: bool,
        y_fn: &impl Fn(Float) -> Float,
    ) -> bool {
        self.x1 = from;
        self.y1 = y_fn(self.x1);
        self.x2 = Self::it_next(self.x1, dx, self.dx_is_absolute, go_down);

        let mut iter_count = 0;
        // x2 is closer to `toward`, x1 is further
        while !Self::it_at_end1(self.x2, toward, go_down) {
            iter_count += 1;
            if iter_count > 100_000 {
                // Failsafe to prevent infinite loops on pathological models
                break;
            }
            self.y2 = y_fn(self.x2);
            if Self::is_found(self.y1, self.y2) {
                return true;
            }
            self.x1 = self.x2;
            self.y1 = self.y2;
            self.x2 = Self::it_next(self.x1, dx, self.dx_is_absolute, go_down);
            
            // Failsafe 2: If x2 didn't advance due to precision limits
            if self.x1 == self.x2 {
                break;
            }
        }

        // Test the boundary
        if !(self.x1 >= toward) {
            self.x2 = toward;
            self.y2 = y_fn(self.x2);
            if Self::is_found(self.y1, self.y2) {
                return true;
            }
        }
        false
    }

    /// Bidirectional search between minimum and maximum.
    ///
    /// Replaces Fortran `SEARCH2(FROM, MINIMUM, MAXIMUM, DX)`.
    fn search2(
        &mut self,
        from: Float,
        minimum: Float,
        maximum: Float,
        dx: Float,
        go_down: bool,
        y_fn: &impl Fn(Float) -> Float,
    ) -> bool {
        self.x1 = from;
        self.y1 = y_fn(self.x1);
        self.y2 = self.polarity;

        // println!("search2: from={}, min={}, max={}, y1={}, polarity={}", from, minimum, maximum, self.y1, self.polarity);

        if Self::is_found(self.y1, self.y2) {
            // eprintln!("Polarity sign change detected! y1={}, polarity={}", self.y1, self.y2);
            if Self::it_at_end2(from, minimum, maximum, go_down) {
                return false;
            }
            self.x2 = self.x1;
            self.y2 = self.y1;
            self.x1 = Self::it_begin(minimum, maximum, go_down);
            self.y1 = self.polarity;
            return true;
        }

        // Root is situated below x0, search by step
        self.x2 = Self::it_next(self.x1, dx, self.dx_is_absolute, go_down);
        let mut iter_count = 0;
        while !Self::it_at_end2(self.x2, minimum, maximum, go_down) {
            iter_count += 1;
            if iter_count > 100_000 {
                break;
            }
            self.y2 = y_fn(self.x2);
            if Self::is_found(self.y1, self.y2) {
                return true;
            }
            self.x1 = self.x2;
            self.y1 = self.y2;
            self.x2 = Self::it_next(self.x1, dx, self.dx_is_absolute, go_down);
            
            if self.x1 == self.x2 {
                break;
            }
        }

        // Test the boundary
        if !Self::it_at_end2(self.x1, minimum, maximum, go_down) {
            self.x2 = Self::it_end(minimum, maximum, go_down);
            self.y2 = y_fn(self.x2);
            if Self::is_found(self.y1, self.y2) {
                return true;
            }
        }
        false
    }
}
