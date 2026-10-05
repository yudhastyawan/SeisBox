//! Physical (monotonic) constraints for PSO / SA / LM / Occam inversion.
//!
//! Parameter vector layout: `[h_0 .. h_{n-2}, vs_0 .. vs_{n-1}]`
//! (thickness of the half-space is not inverted).
//!
//! Two handling modes are supported:
//! - `Penalty`: legacy behaviour, any violating model receives a cost of `1e6`.
//! - `Repair`:  the model is repaired *before* evaluation (sort + clamp for
//!   stochastic methods, isotonic projection + clamp for gradient methods) and
//!   the repaired model is written back to the optimizer state. A graded
//!   penalty is only applied when the bounds themselves are not monotonic.

use serde::{Deserialize, Serialize};

/// Cost returned for violating models in legacy `Penalty` mode.
pub const HARD_PENALTY: f64 = 1e6;
/// Base cost of the graded penalty used in `Repair` mode.
pub const GRADED_PENALTY_BASE: f64 = 1e3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConstraintMode {
    /// Sort + clamp (PSO/SA) or isotonic projection (LM/Occam) before evaluation.
    #[default]
    Repair,
    /// Reject violating models with a hard penalty (legacy).
    Penalty,
}

impl ConstraintMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "repair" | "sort" => Ok(ConstraintMode::Repair),
            "penalty" | "reject" => Ok(ConstraintMode::Penalty),
            other => Err(format!("Unknown constraint mode '{}'. Use 'repair' or 'penalty'.", other)),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ConstraintMode::Repair => "Repair (sort + clamp)",
            ConstraintMode::Penalty => "Penalty (reject)",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Constraints {
    pub nlayers: usize,
    pub inc_vs: bool,
    pub inc_h: bool,
    pub mode: ConstraintMode,
    /// (min, max) per parameter, same layout as the parameter vector.
    pub bounds: Vec<(f64, f64)>,
}

impl Constraints {
    pub fn new(nlayers: usize, inc_vs: bool, inc_h: bool, mode: ConstraintMode, bounds: Vec<(f64, f64)>) -> Self {
        Self { nlayers, inc_vs, inc_h, mode, bounds }
    }

    pub fn is_active(&self) -> bool {
        self.inc_vs || (self.inc_h && self.nlayers > 2)
    }

    fn h_range(&self) -> std::ops::Range<usize> {
        0..self.nlayers.saturating_sub(1)
    }

    fn vs_range(&self) -> std::ops::Range<usize> {
        let n_h = self.nlayers.saturating_sub(1);
        n_h..n_h + self.nlayers
    }

    /// Active (index range) segments that must be non-decreasing.
    fn active_segments(&self) -> Vec<std::ops::Range<usize>> {
        let mut segs = Vec::new();
        if self.inc_h && self.nlayers > 2 {
            segs.push(self.h_range());
        }
        if self.inc_vs && self.nlayers > 1 {
            segs.push(self.vs_range());
        }
        segs
    }

    fn clamp_all(&self, pos: &mut [f64]) {
        for (x, &(lo, hi)) in pos.iter_mut().zip(self.bounds.iter()) {
            *x = x.clamp(lo, hi);
        }
    }

    /// Repair for stochastic methods: sort each active segment ascending, then clamp.
    /// With monotonic bounds the result is guaranteed to stay sorted.
    pub fn repair(&self, pos: &mut [f64]) {
        if !self.is_active() || self.mode != ConstraintMode::Repair {
            return;
        }
        for seg in self.active_segments() {
            pos[seg].sort_by(|a, b| a.total_cmp(b));
        }
        self.clamp_all(pos);
    }

    /// Projection for gradient methods: isotonic regression (PAVA) per active segment,
    /// then clamp. This is the Euclidean projection onto the monotonic cone and keeps
    /// the parameter update continuous (unlike sorting).
    pub fn project(&self, pos: &mut [f64]) {
        if !self.is_active() || self.mode != ConstraintMode::Repair {
            return;
        }
        for seg in self.active_segments() {
            pava_non_decreasing(&mut pos[seg]);
        }
        self.clamp_all(pos);
    }

    /// Total normalized violation of the non-decreasing constraint (0 == feasible).
    pub fn violation(&self, pos: &[f64]) -> f64 {
        let mut total = 0.0;
        for seg in self.active_segments() {
            let lo = seg.clone().map(|i| self.bounds[i].0).fold(f64::INFINITY, f64::min);
            let hi = seg.clone().map(|i| self.bounds[i].1).fold(f64::NEG_INFINITY, f64::max);
            let range = (hi - lo).abs().max(1e-12);
            let s = &pos[seg];
            for w in s.windows(2) {
                total += (w[0] - w[1]).max(0.0) / range;
            }
        }
        total
    }

    /// Strict check used by the legacy penalty mode (`<=` counts as violation).
    pub fn violates_strict(&self, pos: &[f64]) -> bool {
        self.active_segments().into_iter().any(|seg| pos[seg].windows(2).any(|w| w[1] <= w[0]))
    }

    /// Returns `Some(cost)` if the model must be penalized, `None` if it is feasible.
    pub fn penalty(&self, pos: &[f64]) -> Option<f64> {
        if !self.is_active() {
            return None;
        }
        match self.mode {
            ConstraintMode::Penalty => {
                if self.violates_strict(pos) { Some(HARD_PENALTY) } else { None }
            }
            ConstraintMode::Repair => {
                let v = self.violation(pos);
                if v > 0.0 { Some(GRADED_PENALTY_BASE * (1.0 + v)) } else { None }
            }
        }
    }

    /// Warnings when bounds of an active segment are not monotonic. In that case the
    /// repair cannot guarantee feasibility and the graded penalty may be triggered.
    pub fn bounds_warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.is_active() {
            return out;
        }
        let mut check = |name: &str, seg: std::ops::Range<usize>| {
            let b = &self.bounds[seg];
            for (i, w) in b.windows(2).enumerate() {
                if w[1].0 < w[0].0 || w[1].1 < w[0].1 {
                    out.push(format!(
                        "{} bounds are not monotonic between layer {} [{:.1}, {:.1}] and layer {} [{:.1}, {:.1}]",
                        name, i + 1, w[0].0, w[0].1, i + 2, w[1].0, w[1].1
                    ));
                }
            }
        };
        if self.inc_h && self.nlayers > 2 {
            check("Thickness", self.h_range());
        }
        if self.inc_vs && self.nlayers > 1 {
            check("Vs", self.vs_range());
        }
        out
    }
}

/// Check whether a list of `[min, max]` bounds is monotonic (both columns non-decreasing).
pub fn bounds_are_monotonic(bounds: &[[f64; 2]]) -> bool {
    bounds.windows(2).all(|w| w[1][0] >= w[0][0] && w[1][1] >= w[0][1])
}

/// Pool-Adjacent-Violators algorithm (unweighted) for a non-decreasing fit, in place.
pub fn pava_non_decreasing(x: &mut [f64]) {
    if x.len() < 2 {
        return;
    }
    // Blocks of (sum, count)
    let mut sums: Vec<f64> = Vec::with_capacity(x.len());
    let mut counts: Vec<usize> = Vec::with_capacity(x.len());
    for &v in x.iter() {
        sums.push(v);
        counts.push(1);
        while sums.len() > 1 {
            let n = sums.len();
            let prev_mean = sums[n - 2] / counts[n - 2] as f64;
            let last_mean = sums[n - 1] / counts[n - 1] as f64;
            if prev_mean > last_mean {
                let s = sums.pop().unwrap();
                let c = counts.pop().unwrap();
                sums[n - 2] += s;
                counts[n - 2] += c;
            } else {
                break;
            }
        }
    }
    let mut idx = 0;
    for (s, c) in sums.iter().zip(counts.iter()) {
        let mean = s / *c as f64;
        for _ in 0..*c {
            x[idx] = mean;
            idx += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(inc_vs: bool, inc_h: bool, bounds: Vec<(f64, f64)>) -> Constraints {
        // 3 layers -> 2 h + 3 vs
        Constraints::new(3, inc_vs, inc_h, ConstraintMode::Repair, bounds)
    }

    fn monotone_bounds() -> Vec<(f64, f64)> {
        vec![(5.0, 30.0), (10.0, 50.0), (100.0, 300.0), (300.0, 600.0), (500.0, 900.0)]
    }

    #[test]
    fn repair_sorts_vs_only() {
        let c = mk(true, false, vec![(0.0, 100.0); 5]);
        let mut p = vec![20.0, 10.0, 90.0, 30.0, 60.0];
        c.repair(&mut p);
        assert_eq!(p, vec![20.0, 10.0, 30.0, 60.0, 90.0]);
    }

    #[test]
    fn repair_sorts_h_and_vs() {
        let c = mk(true, true, vec![(0.0, 100.0); 5]);
        let mut p = vec![20.0, 10.0, 90.0, 30.0, 60.0];
        c.repair(&mut p);
        assert_eq!(p, vec![10.0, 20.0, 30.0, 60.0, 90.0]);
    }

    #[test]
    fn repair_with_monotone_bounds_is_feasible() {
        let c = mk(true, true, monotone_bounds());
        let mut p = vec![30.0, 10.0, 800.0, 250.0, 550.0];
        c.repair(&mut p);
        assert_eq!(c.violation(&p), 0.0);
        assert!(c.penalty(&p).is_none());
        for (x, (lo, hi)) in p.iter().zip(monotone_bounds()) {
            assert!(*x >= lo && *x <= hi);
        }
    }

    #[test]
    fn non_monotone_bounds_trigger_graded_penalty_and_warning() {
        let b = vec![(5.0, 30.0), (10.0, 50.0), (400.0, 600.0), (100.0, 200.0), (500.0, 900.0)];
        let c = mk(true, false, b);
        let mut p = vec![10.0, 20.0, 450.0, 150.0, 600.0];
        c.repair(&mut p);
        assert!(c.violation(&p) > 0.0);
        let pen = c.penalty(&p).unwrap();
        assert!(pen > GRADED_PENALTY_BASE && pen < HARD_PENALTY);
        assert!(!c.bounds_warnings().is_empty());
    }

    #[test]
    fn pava_basic() {
        let mut x = vec![1.0, 3.0, 2.0, 4.0];
        pava_non_decreasing(&mut x);
        assert_eq!(x, vec![1.0, 2.5, 2.5, 4.0]);
        let mut y = vec![3.0, 2.0, 1.0];
        pava_non_decreasing(&mut y);
        assert_eq!(y, vec![2.0, 2.0, 2.0]);
    }

    #[test]
    fn penalty_mode_is_legacy() {
        let c = Constraints::new(3, true, false, ConstraintMode::Penalty, vec![(0.0, 1000.0); 5]);
        let mut p = vec![10.0, 20.0, 300.0, 200.0, 400.0];
        c.repair(&mut p); // no-op in penalty mode
        assert_eq!(p[3], 200.0);
        assert_eq!(c.penalty(&p), Some(HARD_PENALTY));
    }

    #[test]
    fn inactive_does_nothing() {
        let c = mk(false, false, vec![(0.0, 1000.0); 5]);
        let mut p = vec![30.0, 10.0, 300.0, 200.0, 100.0];
        let orig = p.clone();
        c.repair(&mut p);
        assert_eq!(p, orig);
        assert!(c.penalty(&p).is_none());
    }
}
