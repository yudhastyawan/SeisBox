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
