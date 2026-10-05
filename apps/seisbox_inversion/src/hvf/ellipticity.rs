//! Rayleigh wave ellipticity forward modeling.
//!
//! Translated from Geopsy gen_complex.py / hvsr_forward_complex.cpp.
//! Implements:
//! - Elastic secular function for root finding (fundamental Rayleigh mode)
//! - Anelastic ellipticity with complex arithmetic (Q attenuation support)
//! - Optional Love wave contribution (addLove formula)

use num_complex::Complex64;

/// A simple layer for ellipticity computation.
#[derive(Clone, Debug)]
pub struct EllipLayer {
    pub h: f64,
    pub vp: f64,
    pub vs: f64,
    pub rho: f64,
    pub qp: f64,
    pub qs: f64,
}

/// Result of ellipticity curve computation.
pub struct EllipticityCurveResult {
    pub freqs: Vec<f64>,
    pub hv: Vec<f64>,
    pub slowness: Vec<f64>,
}

/// Compute the elastic secular function for Rayleigh wave.
/// Returns F1212 (the secular determinant), and writes F1213, iF1214 for ellipticity.
pub fn compute_secular_function_elastic(
    slowness: f64,
    omega: f64,
    model: &[EllipLayer],
) -> (f64, f64, f64) {
    let w2 = omega * omega;
    let k = omega * slowness;
    let k2 = k * k;
    let inv_k2 = 1.0 / k2;

    let n = model.len();
    let i = n - 1;

    let ka = omega / model[i].vp;
    let mut hn2 = k2 - ka * ka;
    let mut hn = hn2.abs().sqrt();

    let mut kb = omega / model[i].vs;
    let mut kn2 = k2 - kb * kb;
    let mut kn = kn2.abs().sqrt();

    let ln_val = k2 + kn2;
    let mu = model[i].vs * model[i].vs * model[i].rho;
    let mut c1 = hn * kn;

    let inv_fac = w2;
    let fac = 1.0 / inv_fac;

    let mut f1212 = mu * fac * (ln_val * ln_val - 4.0 * k2 * c1);
    let mut f1213 = hn * (ln_val - 2.0 * k2);
    let mut if1214 = k * (ln_val - 2.0 * c1);
    let mut f1224 = kn * (2.0 * k2 - ln_val);
    let mut f1234 = (k2 - c1) / mu;

    if i == 0 {
        return (f1212, f1213, if1214);
    }

    for ii in (0..i).rev() {
        let ka_i = omega / model[ii].vp;
        hn2 = k2 - ka_i * ka_i;
        hn = hn2.abs().sqrt();
        hn2 *= inv_k2;

        kb = omega / model[ii].vs;
        kn2 = k2 - kb * kb;
        kn = kn2.abs().sqrt();
        kn2 *= inv_k2;

        let dn = model[ii].h;
        let exphn_val = hn * dn;

        let (sh, ch, exphn_out);
        if hn2 > 0.0 {
            if exphn_val < 115.0 {
                let e = (-exphn_val).exp();
                let exp2 = e * e;
                exphn_out = e;
                sh = 0.5 * k / hn * (1.0 - exp2);
                ch = 0.5 * (1.0 + exp2);
            } else {
                exphn_out = 0.0;
                sh = 0.5 * k / hn;
                ch = 0.5;
            }
        } else if hn2 < 0.0 {
            sh = k / hn * exphn_val.sin();
            ch = exphn_val.cos();
            exphn_out = 1.0;
        } else {
            sh = 0.0;
            ch = 1.0;
            exphn_out = 1.0;
        }

        let expkn_val = kn * dn;
        let (sk, ck, expkn_out);
        if kn2 > 0.0 {
            if expkn_val < 21.2 {
                let e = (-expkn_val).exp();
                let exp2 = e * e;
                expkn_out = e;
                sk = 0.5 * k / kn * (1.0 - exp2);
                ck = 0.5 * (1.0 + exp2);
            } else {
                expkn_out = 0.0;
                sk = 0.5 * k / kn;
                ck = 0.5;
            }
        } else if kn2 < 0.0 {
            sk = k / kn * expkn_val.sin();
            ck = expkn_val.cos();
            expkn_out = 1.0;
        } else {
            sk = 0.0;
            ck = 1.0;
            expkn_out = 1.0;
        }

        let exp_corr = exphn_out * expkn_out;
        let chck = ch * ck;
        let shsk = sh * sk;
        let chsk = ch * sk;
        let shck = sh * ck;

        let gn = 2.0 * k2 / (kb * kb);
        let gn2_val = gn * gn;
        let adim1 = gn2_val - 2.0 * gn + 1.0;
        let adim2 = hn2 * kn2;
        let adim3 = gn2_val + adim1;
        let adim4 = 1.0 - gn;
        let adim5 = gn2_val * adim2;
        let chck1 = exp_corr - chck;
        let chck2 = 2.0 * chck1;
        let shck1 = gn * hn2 * shck;

        c1 = model[ii].rho * w2 / k;
        let c2 = 1.0 / c1;

        let g1212 = adim3 * chck - (adim1 + adim5) * shsk - (adim3 - 1.0) * exp_corr;
        let g1213 = c2 * (chsk - hn2 * shck);
        let ig1214 = c2 * ((adim1 - gn2_val) * chck1 + (adim4 - gn * adim2) * shsk);
        let g1224 = c2 * (kn2 * chsk - shck);
        let g1234 = c2 * c2 * (chck2 + (1.0 + adim2) * shsk);
        let g1312 = c1 * (gn2_val * kn2 * chsk - adim1 * shck);
        let g1313 = chck;
        let ig1314 = adim4 * shck + gn * kn2 * chsk;
        let g1324 = kn2 * shsk;
        let g1334 = g1224;
        let ig1412 = c1 * ((adim1 - adim4) * (adim4 - gn) * chck1 + (adim4 * adim1 - gn * adim5) * shsk);
        let ig1413 = shck1 + adim4 * chsk;
        let g1423 = chck - g1212;
        let g1414 = exp_corr + g1423;
        let ig1424 = ig1314;
        let ig1434 = ig1214;
        let g2314 = g1423;
        let g2412 = c1 * (adim1 * chsk - gn * shck1);
        let g2413 = hn2 * shsk;
        let ig2414 = ig1413;
        let g2424 = g1313;
        let g2434 = g1213;
        let g3412 = c1 * c1 * (gn2_val * adim1 * chck2 + (adim1 * adim1 + gn2_val * adim5) * shsk);
        let g3413 = g2412;
        let ig3414 = ig1412;
        let g3424 = g1312;
        let g3434 = g1212;

        let r1212 = f1212 * g1212 + fac * (f1213 * g1312 - 2.0 * if1214 * ig1412 + f1224 * g2412 - f1234 * g3412);
        let r1213 = inv_fac * f1212 * g1213 + f1213 * g1313 + 2.0 * if1214 * ig1413 - f1224 * g2413 + f1234 * g3413;
        let ir1214 = inv_fac * f1212 * ig1214 + f1213 * ig1314 + if1214 * (g1414 + g2314) - f1224 * ig2414 + f1234 * ig3414;
        let r1224 = inv_fac * f1212 * g1224 - f1213 * g1324 - 2.0 * if1214 * ig1424 + f1224 * g2424 + f1234 * g3424;
        let r1234 = f1213 * g1334 - inv_fac * f1212 * g1234 - 2.0 * if1214 * ig1434 + f1224 * g2434 + f1234 * g3434;

        let mut max_r = r1212.abs();
        if r1213.abs() > max_r { max_r = r1213.abs(); }
        if ir1214.abs() > max_r { max_r = ir1214.abs(); }
        if r1224.abs() > max_r { max_r = r1224.abs(); }
        if r1234.abs() > max_r { max_r = r1234.abs(); }

        if max_r > 1.0e5 {
            let scale = 1.0e5 / max_r;
            f1212 = r1212 * scale;
            f1213 = r1213 * scale;
            if1214 = ir1214 * scale;
            f1224 = r1224 * scale;
            f1234 = r1234 * scale;
        } else {
            f1212 = r1212;
            f1213 = r1213;
            if1214 = ir1214;
            f1224 = r1224;
            f1234 = r1234;
        }
    }
    (f1212, f1213, if1214)
}

/// Compute the ellipticity (H/V) from the anelastic propagator matrix.
/// Uses complex arithmetic to support Q attenuation.
/// If `love_alpha` > 0, adds Love wave contribution: H/V = sqrt(ell_R^2 + alpha/(1-alpha))
pub fn compute_ellipticity_anelastic(
    slowness: f64,
    omega: f64,
    model: &[EllipLayer],
    love_alpha: f64,
) -> f64 {
    type C = Complex64;
    let w2 = omega * omega;
    let k = C::new(omega * slowness, 0.0);
    let k2 = k * k;
    let inv_k2 = C::new(1.0, 0.0) / k2;

    let n = model.len();
    let i = n - 1;

    let ka = C::new(
        omega / model[i].vp,
        if model[i].qp > 0.0 { -omega / model[i].vp / (2.0 * model[i].qp) } else { 0.0 },
    );
    let mut hn2 = k2 - ka * ka;
    let mut hn = hn2.sqrt();

    let kb = C::new(
        omega / model[i].vs,
        if model[i].qs > 0.0 { -omega / model[i].vs / (2.0 * model[i].qs) } else { 0.0 },
    );
    let mut kn2 = k2 - kb * kb;
    let mut kn = kn2.sqrt();

    let ln_val = k2 + kn2;
    let mu = model[i].vs * model[i].vs * model[i].rho;
    let mut c1 = hn * kn;

    let inv_fac = w2;
    let fac = 1.0 / inv_fac;

    let mut f1212 = C::new(mu * fac, 0.0) * (ln_val * ln_val - C::new(4.0, 0.0) * k2 * c1);
    let mut f1213 = hn * (ln_val - C::new(2.0, 0.0) * k2);
    let mut if1214 = k * (ln_val - C::new(2.0, 0.0) * c1);
    let mut f1224 = kn * (C::new(2.0, 0.0) * k2 - ln_val);
    let mut f1234 = (k2 - c1) / C::new(mu, 0.0);

    if i > 0 {
        for ii in (0..i).rev() {
            let ka_i = C::new(
                omega / model[ii].vp,
                if model[ii].qp > 0.0 { -omega / model[ii].vp / (2.0 * model[ii].qp) } else { 0.0 },
            );
            hn2 = k2 - ka_i * ka_i;
            hn = hn2.sqrt();
            hn2 = hn2 * inv_k2;

            let kb_i = C::new(
                omega / model[ii].vs,
                if model[ii].qs > 0.0 { -omega / model[ii].vs / (2.0 * model[ii].qs) } else { 0.0 },
            );
            kn2 = k2 - kb_i * kb_i;
            kn = kn2.sqrt();
            kn2 = kn2 * inv_k2;

            let dn = model[ii].h;
            let exphn_arg = hn * C::new(dn, 0.0);

            let (sh, ch, exphn_out): (C, C, C);
            if hn2.re > 0.0 {
                if exphn_arg.re < 115.0 {
                    let e = (-exphn_arg).exp();
                    let exp2 = e * e;
                    exphn_out = e;
                    sh = C::new(0.5, 0.0) * k / hn * (C::new(1.0, 0.0) - exp2);
                    ch = C::new(0.5, 0.0) * (C::new(1.0, 0.0) + exp2);
                } else {
                    exphn_out = C::new(0.0, 0.0);
                    sh = C::new(0.5, 0.0) * k / hn;
                    ch = C::new(0.5, 0.0);
                }
            } else if hn2.re < 0.0 {
                sh = k / hn * exphn_arg.sin();
                ch = exphn_arg.cos();
                exphn_out = C::new(1.0, 0.0);
            } else {
                sh = C::new(0.0, 0.0);
                ch = C::new(1.0, 0.0);
                exphn_out = C::new(1.0, 0.0);
            }

            let expkn_arg = kn * C::new(dn, 0.0);
            let (sk, ck, expkn_out): (C, C, C);
            if kn2.re > 0.0 {
                if expkn_arg.re < 21.2 {
                    let e = (-expkn_arg).exp();
                    let exp2 = e * e;
                    expkn_out = e;
                    sk = C::new(0.5, 0.0) * k / kn * (C::new(1.0, 0.0) - exp2);
                    ck = C::new(0.5, 0.0) * (C::new(1.0, 0.0) + exp2);
                } else {
                    expkn_out = C::new(0.0, 0.0);
                    sk = C::new(0.5, 0.0) * k / kn;
                    ck = C::new(0.5, 0.0);
                }
            } else if kn2.re < 0.0 {
                sk = k / kn * expkn_arg.sin();
                ck = expkn_arg.cos();
                expkn_out = C::new(1.0, 0.0);
            } else {
                sk = C::new(0.0, 0.0);
                ck = C::new(1.0, 0.0);
                expkn_out = C::new(1.0, 0.0);
            }

            let exp_corr = exphn_out * expkn_out;
            let chck = ch * ck;
            let shsk = sh * sk;
            let chsk = ch * sk;
            let shck = sh * ck;

            let gn = C::new(2.0, 0.0) * k2 / (kb_i * kb_i);
            let gn2_val = gn * gn;
            let adim1 = gn2_val - C::new(2.0, 0.0) * gn + C::new(1.0, 0.0);
            let adim2 = hn2 * kn2;
            let adim3 = gn2_val + adim1;
            let adim4 = C::new(1.0, 0.0) - gn;
            let adim5 = gn2_val * adim2;
            let chck1 = exp_corr - chck;
            let chck2 = C::new(2.0, 0.0) * chck1;
            let shck1 = gn * hn2 * shck;

            c1 = C::new(model[ii].rho * w2, 0.0) / k;
            let c2 = C::new(1.0, 0.0) / c1;

            let g1212 = adim3 * chck - (adim1 + adim5) * shsk - (adim3 - C::new(1.0, 0.0)) * exp_corr;
            let g1213 = c2 * (chsk - hn2 * shck);
            let ig1214 = c2 * ((adim1 - gn2_val) * chck1 + (adim4 - gn * adim2) * shsk);
            let g1224 = c2 * (kn2 * chsk - shck);
            let g1234 = c2 * c2 * (chck2 + (C::new(1.0, 0.0) + adim2) * shsk);
            let g1312 = c1 * (gn2_val * kn2 * chsk - adim1 * shck);
            let g1313 = chck;
            let ig1314 = adim4 * shck + gn * kn2 * chsk;
            let g1324 = kn2 * shsk;
            let g1334 = g1224;
            let ig1412 = c1 * ((adim1 - adim4) * (adim4 - gn) * chck1 + (adim4 * adim1 - gn * adim5) * shsk);
            let ig1413 = shck1 + adim4 * chsk;
            let g1423 = chck - g1212;
            let g1414 = exp_corr + g1423;
            let ig1424 = ig1314;
            let ig1434 = ig1214;
            let _g2314 = g1423;
            let g2412 = c1 * (adim1 * chsk - gn * shck1);
            let g2413 = hn2 * shsk;
            let ig2414 = ig1413;
            let g2424 = g1313;
            let g2434 = g1213;
            let g3412 = c1 * c1 * (gn2_val * adim1 * chck2 + (adim1 * adim1 + gn2_val * adim5) * shsk);
            let g3413 = g2412;
            let ig3414 = ig1412;
            let g3424 = g1312;
            let g3434 = g1212;

            let r1212 = f1212 * g1212 + C::new(fac, 0.0) * (f1213 * g1312 - C::new(2.0, 0.0) * if1214 * ig1412 + f1224 * g2412 - f1234 * g3412);
            let r1213 = C::new(inv_fac, 0.0) * f1212 * g1213 + f1213 * g1313 + C::new(2.0, 0.0) * if1214 * ig1413 - f1224 * g2413 + f1234 * g3413;
            let ir1214 = C::new(inv_fac, 0.0) * f1212 * ig1214 + f1213 * ig1314 + if1214 * (g1414 + _g2314) - f1224 * ig2414 + f1234 * ig3414;
            let r1224 = C::new(inv_fac, 0.0) * f1212 * g1224 - f1213 * g1324 - C::new(2.0, 0.0) * if1214 * ig1424 + f1224 * g2424 + f1234 * g3424;
            let r1234 = f1213 * g1334 - C::new(inv_fac, 0.0) * f1212 * g1234 - C::new(2.0, 0.0) * if1214 * ig1434 + f1224 * g2434 + f1234 * g3434;

            let mut max_r = r1212.norm();
            if r1213.norm() > max_r { max_r = r1213.norm(); }
            if ir1214.norm() > max_r { max_r = ir1214.norm(); }
            if r1224.norm() > max_r { max_r = r1224.norm(); }
            if r1234.norm() > max_r { max_r = r1234.norm(); }

            if max_r > 1.0e5 {
                let scale = C::new(1.0e5 / max_r, 0.0);
                f1212 = r1212 * scale;
                f1213 = r1213 * scale;
                if1214 = ir1214 * scale;
                f1224 = r1224 * scale;
                f1234 = r1234 * scale;
            } else {
                f1212 = r1212;
                f1213 = r1213;
                if1214 = ir1214;
                f1224 = r1224;
                f1234 = r1234;
            }
        }
    }

    let u_z = f1213;
    let u_x = if1214;

    if u_z.norm() < 1e-30 {
        return 1e30;
    }

    let hv_rayleigh_sq = u_x.norm_sqr() / u_z.norm_sqr();

    if love_alpha > 0.0 && love_alpha < 1.0 {
        return (hv_rayleigh_sq + love_alpha / (1.0 - love_alpha)).sqrt();
    }

    hv_rayleigh_sq.sqrt()
}

/// Find the fundamental Rayleigh mode slowness at a given frequency via bisection.
pub fn find_fundamental_mode(
    freq: f64,
    model: &[EllipLayer],
    min_vs: f64,
    max_vs: f64,
) -> Option<f64> {
    let omega = 2.0 * std::f64::consts::PI * freq;

    let min_slowness = 0.85 / max_vs;
    let max_slowness = 1.1 / min_vs;

    let n_steps = 1000;
    let step = (max_slowness - min_slowness) / n_steps as f64;

    let mut prev_s = max_slowness;
    let (prev_det, _, _) = compute_secular_function_elastic(prev_s, omega, model);
    let mut prev_f = prev_det;

    for i in 1..=n_steps {
        let s = max_slowness - i as f64 * step;
        let (f_val, _, _) = compute_secular_function_elastic(s, omega, model);

        if (prev_f > 0.0 && f_val < 0.0) || (prev_f < 0.0 && f_val > 0.0) {
            // Bisection refinement
            let mut s1 = prev_s;
            let mut s2 = s;
            let mut f2 = f_val;
            for _ in 0..50 {
                let sm = (s1 + s2) / 2.0;
                let (fm, _, _) = compute_secular_function_elastic(sm, omega, model);
                if (fm > 0.0 && f2 > 0.0) || (fm < 0.0 && f2 < 0.0) {
                    s2 = sm;
                    f2 = fm;
                } else {
                    s1 = sm;
                }
            }
            return Some((s1 + s2) / 2.0);
        }
        prev_f = f_val;
        prev_s = s;
    }
    None
}

/// Compute a full ellipticity curve over a frequency range.
pub fn compute_ellipticity_curve(
    model: &[EllipLayer],
    freqs: &[f64],
    love_alpha: f64,
) -> EllipticityCurveResult {
    let mut min_vs = f64::MAX;
    let mut max_vs = 0.0_f64;
    for l in model {
        if l.vs < min_vs { min_vs = l.vs; }
        if l.vs > max_vs { max_vs = l.vs; }
    }

    let mut hv = Vec::with_capacity(freqs.len());
    let mut slowness_out = Vec::with_capacity(freqs.len());

    for &freq in freqs {
        match find_fundamental_mode(freq, model, min_vs, max_vs) {
            Some(root_slow) => {
                let omega = 2.0 * std::f64::consts::PI * freq;
                let ell = compute_ellipticity_anelastic(root_slow, omega, model, love_alpha);
                hv.push(ell);
                slowness_out.push(root_slow);
            }
            None => {
                hv.push(f64::NAN);
                slowness_out.push(f64::NAN);
            }
        }
    }

    EllipticityCurveResult {
        freqs: freqs.to_vec(),
        hv,
        slowness: slowness_out,
    }
}
