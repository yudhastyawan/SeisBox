use num_complex::Complex64;
use std::f64::consts::PI;

use crate::hvf::model::EarthModel;
use crate::hvf::cli::Config;

/// Computes the transfer function (amplification) for a given velocity profile.
/// Equivalent to the `AMP` function in `hvratio.py`.
fn calculate_amp(
    vel: &[f64],
    density: &[f64],
    h: &[f64],
    q: &[f64],
    k_q: f64,
    f_ref: f64,
    freqs: &[f64],
) -> Vec<f64> {
    let len_v = vel.len();
    let len_f = freqs.len();

    let mut amp = vec![0.0; len_f];
    let mut a = vec![Complex64::new(0.0, 0.0); len_v];
    let mut b = vec![Complex64::new(0.0, 0.0); len_v];
    let mut s = vec![Complex64::new(0.0, 0.0); len_v];
    let mut alpha = vec![Complex64::new(0.0, 0.0); len_v];

    a[0] = Complex64::new(1.0, 0.0);
    b[0] = Complex64::new(1.0, 0.0);

    let j_imag = Complex64::new(0.0, 1.0);

    for i in 0..len_f {
        let f = freqs[i];

        for j in 1..len_v {
            let (vj, vjm1) = if f_ref != 0.0 {
                let qf_kq = q[j] * f.powf(k_q);
                let qf_kq_inv2 = (qf_kq).powi(-2);
                
                let fac_real = 2.0 / (1.0 + (1.0 + qf_kq_inv2).sqrt());
                let fac_cmplx = Complex64::new(1.0, -1.0 / qf_kq);
                let fac = (fac_real * fac_cmplx).sqrt();

                let qf_kq_m1 = q[j - 1] * f.powf(k_q);
                let qf_kq_inv2_m1 = (qf_kq_m1).powi(-2);
                
                let fac_real_m1 = 2.0 / (1.0 + (1.0 + qf_kq_inv2_m1).sqrt());
                let fac_cmplx_m1 = Complex64::new(1.0, -1.0 / qf_kq_m1);
                let facm1 = (fac_real_m1 * fac_cmplx_m1).sqrt();

                let log_term = 1.0 + 1.0 / (PI * qf_kq) * (f / f_ref).ln();
                let log_term_m1 = 1.0 + 1.0 / (PI * qf_kq_m1) * (f / f_ref).ln();

                let vj = Complex64::new(vel[j] * log_term, 0.0) / fac;
                let vjm1 = Complex64::new(vel[j - 1] * log_term_m1, 0.0) / facm1;
                
                (vj, vjm1)
            } else {
                (Complex64::new(vel[j], 0.0), Complex64::new(vel[j - 1], 0.0))
            };

            // Impedance ratio
            alpha[j - 1] = (Complex64::new(density[j - 1], 0.0) * vjm1) / (Complex64::new(density[j], 0.0) * vj);

            // s = k * H -> k = w / c
            s[j - 1] = (Complex64::new(2.0 * PI * f * h[j - 1], 0.0)) / vjm1;

            let exp_pos = (j_imag * s[j - 1]).exp();
            let exp_neg = (-j_imag * s[j - 1]).exp();

            let a_prev = a[j - 1];
            let b_prev = b[j - 1];

            a[j] = 0.5 * ((Complex64::new(1.0, 0.0) + alpha[j - 1]) * exp_pos * a_prev + (Complex64::new(1.0, 0.0) - alpha[j - 1]) * exp_neg * b_prev);
            b[j] = 0.5 * ((Complex64::new(1.0, 0.0) - alpha[j - 1]) * exp_pos * a_prev + (Complex64::new(1.0, 0.0) + alpha[j - 1]) * exp_neg * b_prev);
        }

        amp[i] = 1.0 / a[len_v - 1].norm();
    }

    amp
}

/// Computes the H/V ratio using the Herak (2008) formulation.
pub fn compute_hvsr_herak(model: &EarthModel, freqs: &[f64], config: &Config) -> Vec<f64> {
    let len_v = model.layers.len();
    
    let mut vp = Vec::with_capacity(len_v);
    let mut vs = Vec::with_capacity(len_v);
    let mut density = Vec::with_capacity(len_v);
    let mut h = Vec::with_capacity(len_v);
    let mut qp = Vec::with_capacity(len_v);
    let mut qs = Vec::with_capacity(len_v);

    for layer in &model.layers {
        vp.push(layer.vp);         // Vp
        vs.push(layer.vs);         // Vs
        density.push(layer.density); // rho
        h.push(layer.thickness);   // thickness
        qp.push(layer.qp.unwrap_or(config.qp));
        qs.push(layer.qs.unwrap_or(config.qs));
    }

    let amp_p = calculate_amp(&vp, &density, &h, &qp, config.kq, config.fref, freqs);
    let amp_s = calculate_amp(&vs, &density, &h, &qs, config.kq, config.fref, freqs);

    let mut hvsr = Vec::with_capacity(freqs.len());
    for i in 0..freqs.len() {
        hvsr.push(amp_s[i] / amp_p[i]);
    }

    hvsr
}
