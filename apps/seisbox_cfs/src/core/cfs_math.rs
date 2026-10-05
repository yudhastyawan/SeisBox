use std::f64::consts::PI;

pub fn coord_conversion_single(xgg: f64, ygg: f64, xs: f64, ys: f64, xf: f64, yf: f64, top: f64, bottom: f64, dip: f64) -> (f64, f64, f64, f64) {
    let mut cx = (xf + xs) / 2.0;
    let mut cy = (yf + ys) / 2.0;
    let h = (bottom - top) / 2.0;

    let mut k = dip.to_radians().tan();
    if k == 0.0 { k = 0.000001; }
    let d = h / k;

    let dx = xf - xs;
    let dy = yf - ys;
    let b = if dx == 0.0 {
        (PI / 2.0) * dy.signum()
    } else {
        (dy / dx).atan()
    };

    let ydipshift = (d * b.cos()).abs();
    let xdipshift = (d * b.sin()).abs();

    if xf > xs {
        if yf > ys {
            cx += xdipshift;
            cy -= ydipshift;
        } else {
            cx -= xdipshift;
            cy -= ydipshift;
        }
    } else {
        if yf > ys {
            cx += xdipshift;
            cy += ydipshift;
        } else {
            cx -= xdipshift;
            cy += ydipshift;
        }
    }

    let mut xn = (xgg - cx) * b.cos() + (ygg - cy) * b.sin();
    let mut yn = -(xgg - cx) * b.sin() + (ygg - cy) * b.cos();

    if dx < 0.0 {
        xn = -xn;
        yn = -yn;
    }

    let al = ((dx * dx + dy * dy).sqrt()) / 2.0;
    let aw = ((bottom - top) / 2.0) / dip.to_radians().sin();

    (xn, yn, al, aw)
}

pub fn tensor_trans_single(sinb: f64, cosb: f64, so: &[f64; 6]) -> [f64; 6] {
    let ver = PI / 2.0;
    let bt = sinb.asin();
    
    let (xbeta, xdel, ybeta, ydel, zbeta, zdel): (f64, f64, f64, f64, f64, f64) = if cosb > 0.0 {
        (-bt, 0.0, -bt + ver, 0.0, -bt - ver, ver)
    } else {
        (bt - PI, 0.0, bt - ver, 0.0, bt - ver, ver)
    };
    
    let xl = xdel.cos() * xbeta.cos();
    let xm = xdel.cos() * xbeta.sin();
    let xn = xdel.sin();
    
    let yl = ydel.cos() * ybeta.cos();
    let ym = ydel.cos() * ybeta.sin();
    let yn = ydel.sin();
    
    let zl = zdel.cos() * zbeta.cos();
    let zm = zdel.cos() * zbeta.sin();
    let zn = zdel.sin();
    
    let mut t = [[0.0; 6]; 6];
    
    t[0][0] = xl * xl; t[0][1] = xm * xm; t[0][2] = xn * xn;
    t[0][3] = 2.0 * xm * xn; t[0][4] = 2.0 * xn * xl; t[0][5] = 2.0 * xl * xm;
    
    t[1][0] = yl * yl; t[1][1] = ym * ym; t[1][2] = yn * yn;
    t[1][3] = 2.0 * ym * yn; t[1][4] = 2.0 * yn * yl; t[1][5] = 2.0 * yl * ym;
    
    t[2][0] = zl * zl; t[2][1] = zm * zm; t[2][2] = zn * zn;
    t[2][3] = 2.0 * zm * zn; t[2][4] = 2.0 * zn * zl; t[2][5] = 2.0 * zl * zm;
    
    t[3][0] = yl * zl; t[3][1] = ym * zm; t[3][2] = yn * zn;
    t[3][3] = ym * zn + zm * yn; t[3][4] = yn * zl + zn * yl; t[3][5] = yl * zm + zl * ym;
    
    t[4][0] = zl * xl; t[4][1] = zm * xm; t[4][2] = zn * xn;
    t[4][3] = xm * zn + zm * xn; t[4][4] = xn * zl + zn * xl; t[4][5] = xl * zm + zl * xm;
    
    t[5][0] = xl * yl; t[5][1] = xm * ym; t[5][2] = xn * yn;
    t[5][3] = xm * yn + ym * xn; t[5][4] = xn * yl + yn * xl; t[5][5] = xl * ym + yl * xm;
    
    let mut sn = [0.0; 6];
    for i in 0..6 {
        for j in 0..6 {
            sn[i] += t[i][j] * so[j];
        }
    }
    
    sn
}

pub fn calc_coulomb_single(strike_m: f64, dip_m: f64, rake_m: f64, friction: f64, ss: &[f64; 6]) -> (f64, f64, f64) {
    let mut strike = if strike_m >= 180.0 { strike_m - 180.0 } else { strike_m };
    let mut dip = if strike_m >= 180.0 { -dip_m } else { dip_m };
    
    let rake_adjusted = rake_m - 90.0;
    let rake = if rake_adjusted <= -180.0 { 360.0 + rake_adjusted } else { rake_adjusted };
    
    strike = strike.to_radians();
    dip = dip.to_radians();
    let rake_rad = rake.to_radians();
    
    let rsc = -rake_rad;
    let c_a = rsc.cos();
    let s_a = rsc.sin();
    
    let mtran = [
        [1.0, 0.0, 0.0],
        [0.0, c_a, -s_a],
        [0.0, s_a, c_a],
    ];
    
    let ver = PI / 2.0;
    let c1 = strike >= 0.0;
    let c2 = strike < 0.0;
    let c3 = strike <= ver;
    let c4 = strike > ver;
    let c24 = c2 || c4;
    let d1 = dip >= 0.0;
    let d2 = dip < 0.0;
    
    let xbeta = if d1 { -strike } else { PI - strike };
    let ybeta = if d1 { PI - strike } else { -strike };
    let zbeta = if d1 { ver - strike } else if c1 && c3 { -ver - strike } else if c24 { PI + ver - strike } else { 0.0 };
    
    let xdel = (ver - dip.abs()) as f64;
    let ydel = dip.abs() as f64;
    let zdel = 0.0f64;
    
    let xl = xdel.cos() * xbeta.cos();
    let xm = xdel.cos() * xbeta.sin();
    let xn = xdel.sin();
    
    let yl = ydel.cos() * ybeta.cos();
    let ym = ydel.cos() * ybeta.sin();
    let yn = ydel.sin();
    
    let zl = zdel.cos() * zbeta.cos();
    let zm = zdel.cos() * zbeta.sin();
    let zn = zdel.sin();
    
    let mut t = [[0.0; 6]; 6];
    t[0][0] = xl * xl; t[0][1] = xm * xm; t[0][2] = xn * xn;
    t[0][3] = 2.0 * xm * xn; t[0][4] = 2.0 * xn * xl; t[0][5] = 2.0 * xl * xm;
    
    t[1][0] = yl * yl; t[1][1] = ym * ym; t[1][2] = yn * yn;
    t[1][3] = 2.0 * ym * yn; t[1][4] = 2.0 * yn * yl; t[1][5] = 2.0 * yl * ym;
    
    t[2][0] = zl * zl; t[2][1] = zm * zm; t[2][2] = zn * zn;
    t[2][3] = 2.0 * zm * zn; t[2][4] = 2.0 * zn * zl; t[2][5] = 2.0 * zl * zm;
    
    t[3][0] = yl * zl; t[3][1] = ym * zm; t[3][2] = yn * zn;
    t[3][3] = ym * zn + zm * yn; t[3][4] = yn * zl + zn * yl; t[3][5] = yl * zm + zl * ym;
    
    t[4][0] = zl * xl; t[4][1] = zm * xm; t[4][2] = zn * xn;
    t[4][3] = xm * zn + zm * xn; t[4][4] = xn * zl + zn * xl; t[4][5] = xl * zm + zl * xm;
    
    t[5][0] = xl * yl; t[5][1] = xm * ym; t[5][2] = xn * yn;
    t[5][3] = xm * yn + ym * xn; t[5][4] = xn * yl + yn * xl; t[5][5] = xl * ym + yl * xm;
    
    let mut sn = [0.0; 6];
    for i in 0..6 {
        for j in 0..6 {
            sn[i] += t[i][j] * ss[j];
        }
    }
    
    let mut sn9 = [
        [sn[0], sn[5], sn[4]],
        [sn[5], sn[1], sn[3]],
        [sn[4], sn[3], sn[2]],
    ];
    
    let mut sn9_rot = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                sn9_rot[i][j] += sn9[i][k] * mtran[k][j];
            }
        }
    }
    
    let shear = sn9_rot[0][1];
    let normal = sn9_rot[0][0];
    let coulomb = shear + friction * normal;
    
    (shear, normal, coulomb)
}

/// Jacobi Eigenvalue Algorithm for 3x3 symmetric matrix
pub fn jacobi_eigenvalue_3x3(a: &[[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut d = *a;
    let max_iter = 50;
    
    for _ in 0..max_iter {
        let mut p = 0;
        let mut q = 1;
        let mut max_val = d[0][1].abs();
        
        if d[0][2].abs() > max_val { max_val = d[0][2].abs(); p = 0; q = 2; }
        if d[1][2].abs() > max_val { max_val = d[1][2].abs(); p = 1; q = 2; }
        
        if max_val < 1e-12 {
            break;
        }
        
        let diff = d[p][p] - d[q][q];
        let theta = if diff.abs() < 1e-15 {
            if d[p][q] > 0.0 { std::f64::consts::PI / 4.0 } else { -std::f64::consts::PI / 4.0 }
        } else {
            0.5 * (2.0 * d[p][q]).atan2(diff)
        };
        
        let c = theta.cos();
        let s = theta.sin();
        
        let mut d_new = d;
        d_new[p][p] = c * c * d[p][p] + 2.0 * s * c * d[p][q] + s * s * d[q][q];
        d_new[q][q] = s * s * d[p][p] - 2.0 * s * c * d[p][q] + c * c * d[q][q];
        d_new[p][q] = 0.0;
        d_new[q][p] = 0.0;
        
        for r in 0..3 {
            if r != p && r != q {
                d_new[r][p] = c * d[r][p] + s * d[r][q];
                d_new[p][r] = d_new[r][p];
                d_new[r][q] = -s * d[r][p] + c * d[r][q];
                d_new[q][r] = d_new[r][q];
            }
        }
        d = d_new;
        
        for r in 0..3 {
            let v_rp = v[r][p];
            let v_rq = v[r][q];
            v[r][p] = c * v_rp + s * v_rq;
            v[r][q] = -s * v_rp + c * v_rq;
        }
    }
    
    // Sort eigenvalues and corresponding eigenvectors in descending order
    let mut eigs = [(d[0][0], 0), (d[1][1], 1), (d[2][2], 2)];
    eigs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    
    let mut sorted_d = [0.0; 3];
    let mut sorted_v = [[0.0; 3]; 3];
    
    for i in 0..3 {
        sorted_d[i] = eigs[i].0;
        let orig_idx = eigs[i].1;
        for j in 0..3 {
            sorted_v[j][i] = v[j][orig_idx];
        }
    }
    
    (sorted_d, sorted_v)
}

/// Build a 3x3 regional stress tensor from parameters typically found in Coulomb INP files.
///
/// Convention (Coulomb 3.3):
///   - Coordinate system: X = East, Y = North, Z = Up (positive upward)
///   - `magnitude`: differential stress σ₁ − σ₃ (in bars)
///   - `azimuth_deg`: azimuth of σ₁ direction (degrees CW from North)
///   - `plunge_deg`: plunge of σ₁ (degrees downward from horizontal)
///   - The middle principal stress σ₂ is set to the average of σ₁ and σ₃ (R = 0.5)
///
/// We define σ₁ = +magnitude/2 (most compressive in geoscience tension-positive convention
/// maps to most negative, but Coulomb uses compression-negative in the stress tensor),
/// σ₃ = −magnitude/2, σ₂ = 0 so that σ₁ − σ₃ = magnitude.
///
/// Returns a 3×3 symmetric tensor in (X=East, Y=North, Z=Up) frame.
pub fn build_regional_stress_tensor(magnitude: f64, azimuth_deg: f64, plunge_deg: f64) -> [[f64; 3]; 3] {
    let az = azimuth_deg.to_radians();
    let pl = plunge_deg.to_radians();

    // σ₁ direction unit vector (East, North, Up)
    // azimuth measured CW from North: East = sin(az), North = cos(az)
    // plunge is downward from horizontal: Up component = -sin(pl)
    let d1 = [
        az.sin() * pl.cos(),   // East
        az.cos() * pl.cos(),   // North
        -pl.sin(),             // Up (plunge goes down)
    ];

    // σ₃ is perpendicular to σ₁ in the vertical plane containing σ₁ (for the simple
    // Coulomb 3.3 parameterisation where only azimuth+plunge of σ₁ is given).
    // σ₃ direction: rotate σ₁ by 90° toward vertical
    let d3 = [
        -az.sin() * pl.sin(),
        -az.cos() * pl.sin(),
        -pl.cos(),
    ];

    // σ₂ is horizontal and perpendicular to σ₁'s horizontal projection
    // (cross product of d1 × d3 gives the third axis, but for the simple
    // parameterisation we derive it directly)
    let d2 = [
        d1[1] * d3[2] - d1[2] * d3[1],
        d1[2] * d3[0] - d1[0] * d3[2],
        d1[0] * d3[1] - d1[1] * d3[0],
    ];

    // Principal values: σ₁ = +mag/2, σ₂ = 0, σ₃ = -mag/2
    // so that σ₁ - σ₃ = magnitude
    let s1 = magnitude / 2.0;
    let s2 = 0.0;
    let s3 = -magnitude / 2.0;

    // Reconstruct full tensor: σ = Σ_k  s_k · (d_k ⊗ d_k)
    let mut tensor = [[0.0f64; 3]; 3];
    let dirs = [d1, d2, d3];
    let vals = [s1, s2, s3];

    for k in 0..3 {
        for i in 0..3 {
            for j in 0..3 {
                tensor[i][j] += vals[k] * dirs[k][i] * dirs[k][j];
            }
        }
    }

    tensor
}

/// Convert a 3×3 symmetric tensor to 6-component Voigt notation [sxx, syy, szz, syz, sxz, sxy].
fn tensor_3x3_to_voigt(t: &[[f64; 3]; 3]) -> [f64; 6] {
    [t[0][0], t[1][1], t[2][2], t[1][2], t[0][2], t[0][1]]
}

/// Add two 6-component Voigt stress tensors.
pub fn add_voigt(a: &[f64; 6], b: &[f64; 6]) -> [f64; 6] {
    [
        a[0] + b[0], a[1] + b[1], a[2] + b[2],
        a[3] + b[3], a[4] + b[4], a[5] + b[5],
    ]
}

/// Convert regional tensor (3×3) to Voigt (6-component).
pub fn regional_tensor_to_voigt(t: &[[f64; 3]; 3]) -> [f64; 6] {
    tensor_3x3_to_voigt(t)
}

/// Determine the two conjugate Optimally Oriented Fault (OOF) planes from the
/// **total** stress tensor (regional + coseismic perturbation) using Mohr-Coulomb
/// failure theory.
///
/// Steps:
/// 1. Eigen-decompose the total stress tensor to get principal stresses and axes.
/// 2. The optimal fault plane normal lies at angle θ = π/4 + φ/2 from σ₃ toward σ₁,
///    where φ = arctan(friction). Two conjugate planes exist (±θ rotation).
/// 3. The slip direction (rake) is determined by resolving the shear traction on
///    each plane.
///
/// Returns exactly 2 candidates: `[(strike1, dip1, rake1), (strike2, dip2, rake2)]`.
pub fn optimal_fault_from_total_stress(total_stress: &[f64; 6], friction: f64) -> [(f64, f64, f64); 2] {
    let sxx = total_stress[0]; let syy = total_stress[1]; let szz = total_stress[2];
    let syz = total_stress[3]; let sxz = total_stress[4]; let sxy = total_stress[5];

    let matrix = [
        [sxx, sxy, sxz],
        [sxy, syy, syz],
        [sxz, syz, szz],
    ];

    let (evals, evecs) = jacobi_eigenvalue_3x3(&matrix);

    // Eigenvectors are sorted descending: evals[0] >= evals[1] >= evals[2]
    // In geology sign convention (tension positive):
    //   σ₁ = evals[0] (most tensile / least compressive)
    //   σ₃ = evals[2] (most compressive)
    // The OOF is at angle θ from σ₃ (most compressive axis) toward σ₁.
    let v1 = [evecs[0][0], evecs[1][0], evecs[2][0]]; // σ₁ eigenvector
    let v3 = [evecs[0][2], evecs[1][2], evecs[2][2]]; // σ₃ eigenvector

    let phi = friction.atan();
    // Angle between fault normal and σ₃ (most compressive)
    let theta = PI / 4.0 + phi / 2.0;
    let cos_t = theta.cos();
    let sin_t = theta.sin();

    // Two conjugate fault-plane normals
    let normals = [
        [
            cos_t * v3[0] + sin_t * v1[0],
            cos_t * v3[1] + sin_t * v1[1],
            cos_t * v3[2] + sin_t * v1[2],
        ],
        [
            cos_t * v3[0] - sin_t * v1[0],
            cos_t * v3[1] - sin_t * v1[1],
            cos_t * v3[2] - sin_t * v1[2],
        ],
    ];

    let mut results = [(0.0f64, 0.0f64, 0.0f64); 2];

    for (idx, n_raw) in normals.iter().enumerate() {
        // Normalise (should already be unit, but be safe)
        let mag = (n_raw[0]*n_raw[0] + n_raw[1]*n_raw[1] + n_raw[2]*n_raw[2]).sqrt();
        let mut n = [n_raw[0]/mag, n_raw[1]/mag, n_raw[2]/mag];

        // Convention: fault normal should point upward (nz > 0) so that dip ∈ [0, 90].
        // In our frame Z = Up, so upward normal means n[2] > 0.
        if n[2] < 0.0 {
            n[0] = -n[0]; n[1] = -n[1]; n[2] = -n[2];
        }

        // Dip = angle between normal and vertical (Z axis)
        let dip = n[2].acos().to_degrees();

        // Dip direction = azimuth of the horizontal projection of the normal
        // atan2(East, North) gives azimuth CW from North
        let dip_dir = n[0].atan2(n[1]).to_degrees();
        let mut strike = dip_dir - 90.0;
        if strike < 0.0 { strike += 360.0; }
        if strike >= 360.0 { strike -= 360.0; }

        // Compute rake from resolved shear traction on the fault plane.
        // Traction vector: t_i = Σ_j σ_ij · n_j
        let traction = [
            sxx * n[0] + sxy * n[1] + sxz * n[2],
            sxy * n[0] + syy * n[1] + syz * n[2],
            sxz * n[0] + syz * n[1] + szz * n[2],
        ];

        // Shear traction = traction - (traction · n) · n
        let t_dot_n = traction[0]*n[0] + traction[1]*n[1] + traction[2]*n[2];
        let shear_trac = [
            traction[0] - t_dot_n * n[0],
            traction[1] - t_dot_n * n[1],
            traction[2] - t_dot_n * n[2],
        ];

        // Slip direction = direction of shear traction (the fault slips in the direction
        // of maximum shear stress on the plane).
        let shear_mag = (shear_trac[0]*shear_trac[0] + shear_trac[1]*shear_trac[1] + shear_trac[2]*shear_trac[2]).sqrt();

        let rake = if shear_mag > 1e-15 {
            let slip = [shear_trac[0]/shear_mag, shear_trac[1]/shear_mag, shear_trac[2]/shear_mag];

            // Express rake in Aki & Richards convention:
            // strike direction vector
            let strike_rad = strike.to_radians();
            let strike_vec = [strike_rad.sin(), strike_rad.cos(), 0.0];

            // dip direction vector (horizontal, 90° CW from strike)
            let dip_dir_rad = (strike + 90.0).to_radians();
            // updip vector on the fault plane
            let updip = [
                dip_dir_rad.sin() * dip.to_radians().cos(),
                dip_dir_rad.cos() * dip.to_radians().cos(),
                dip.to_radians().sin(),
            ];

            // rake = atan2(slip · updip, slip · strike_vec)
            let slip_along_strike = slip[0]*strike_vec[0] + slip[1]*strike_vec[1] + slip[2]*strike_vec[2];
            let slip_along_updip = slip[0]*updip[0] + slip[1]*updip[1] + slip[2]*updip[2];
            slip_along_updip.atan2(slip_along_strike).to_degrees()
        } else {
            0.0 // degenerate case: no shear stress on plane
        };

        results[idx] = (strike, dip, rake);
    }

    results
}

