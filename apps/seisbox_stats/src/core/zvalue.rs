use crate::core::catalogue::Catalogue;

/// Calculates the standard normal deviate (Z-value) to measure seismic quiescence/activation.
/// 
/// Compares the seismicity rate in a background window (T1) against a foreground window (T2).
/// Formula: Z = (R1 - R2) / sqrt(R1/T1 + R2/T2)
/// 
/// Returns: (Z-value, R1, R2, N1, N2)
pub fn calculate_zvalue(
    catalog: &Catalogue,
    t1_start: i64,
    t1_end: i64,
    t2_start: i64,
    t2_end: i64,
) -> Option<(f64, f64, f64, usize, usize)> {
    let mut n1 = 0;
    let mut n2 = 0;

    for ev in &catalog.events {
        let ts = ev.time.and_utc().timestamp();
        if ts >= t1_start && ts <= t1_end {
            n1 += 1;
        }
        if ts >= t2_start && ts <= t2_end {
            n2 += 1;
        }
    }

    let t1_duration = (t1_end - t1_start) as f64 / 86400.0 / 365.25; // in years
    let t2_duration = (t2_end - t2_start) as f64 / 86400.0 / 365.25; // in years

    if t1_duration <= 0.0 || t2_duration <= 0.0 {
        return None;
    }

    let r1 = n1 as f64 / t1_duration;
    let r2 = n2 as f64 / t2_duration;

    if r1 == 0.0 && r2 == 0.0 {
        return None; // Cannot calculate variance if both rates are zero
    }

    let variance = (r1 / t1_duration) + (r2 / t2_duration);
    let z = (r1 - r2) / variance.sqrt();

    Some((z, r1, r2, n1, n2))
}
