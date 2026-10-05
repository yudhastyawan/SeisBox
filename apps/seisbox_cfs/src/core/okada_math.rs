use std::f64::consts::PI;

pub struct Dccon0 {
    pub alp1: f64,
    pub alp2: f64,
    pub alp3: f64,
    pub alp4: f64,
    pub alp5: f64,
    pub sd: f64,
    pub cd: f64,
    pub sdsd: f64,
    pub cdcd: f64,
    pub sdcd: f64,
    pub s2d: f64,
    pub c2d: f64,
}

pub fn dccon0(alpha: f64, dip: f64) -> Dccon0 {
    let f1 = 1.0;
    let f2 = 2.0;
    let eps = 1.0e-6;

    let alp1 = (f1 - alpha) / f2;
    let alp2 = alpha / f2;
    let alp3 = (f1 - alpha) / alpha;
    let alp4 = f1 - alpha;
    let alp5 = alpha;

    let p18 = (2.0 * PI) / 360.0;
    let mut sd = (dip * p18).sin();
    let mut cd = (dip * p18).cos();

    if cd.abs() < eps {
        cd = 0.0;
        if sd > 0.0 { sd = 1.0; }
        else if sd < 0.0 { sd = -1.0; }
    }

    Dccon0 {
        alp1, alp2, alp3, alp4, alp5,
        sd, cd,
        sdsd: sd * sd,
        cdcd: cd * cd,
        sdcd: sd * cd,
        s2d: f2 * sd * cd,
        c2d: cd * cd - sd * sd,
    }
}

pub struct Dccon2 {
    pub r: f64,
    pub r2: f64,
    pub r3: f64,
    pub r5: f64,
    pub y: f64,
    pub d: f64,
    pub tt: f64,
    pub alx: f64,
    pub ale: f64,
    pub x11: f64,
    pub y11: f64,
    pub x32: f64,
    pub y32: f64,
    pub ey: f64,
    pub ez: f64,
    pub fy: f64,
    pub fz: f64,
    pub gy: f64,
    pub gz: f64,
    pub hy: f64,
    pub hz: f64,
    pub xi2: f64,
    pub et2: f64,
    pub q2: f64,
}

pub fn dccon2(mut xi: f64, mut et: f64, mut q: f64, sd: f64, cd: f64) -> Dccon2 {
    let eps = 1.0e-6;
    if xi.abs() < eps { xi = 0.0; }
    if et.abs() < eps { et = 0.0; }
    if q.abs() < eps { q = 0.0; }

    let xi2 = xi * xi;
    let et2 = et * et;
    let q2 = q * q;
    let r2 = xi2 + et2 + q2;
    let r = r2.sqrt();
    let r3 = r * r2;
    let r5 = r3 * r2;

    let y = et * cd + q * sd;
    let d = et * sd - q * cd;

    let tt = if q == 0.0 {
        0.0
    } else {
        (xi * et / (q * r)).atan()
    };

    let rxi = r + xi;
    let alx = if xi < 0.0 && q == 0.0 && et == 0.0 {
        -( (r - xi).max(1e-12).ln() )
    } else {
        rxi.max(1e-12).ln()
    };
    
    let x11 = if xi < 0.0 && q == 0.0 && et == 0.0 {
        0.0
    } else {
        1.0 / (r * rxi.max(1e-12))
    };

    let x32 = if xi < 0.0 && q == 0.0 && et == 0.0 {
        0.0
    } else {
        (r + rxi) * x11 * x11 / r.max(1e-12)
    };

    let ret = r + et;
    let ale = if et < 0.0 && q == 0.0 && xi == 0.0 {
        -( (r - et).max(1e-12).ln() )
    } else {
        ret.max(1e-12).ln()
    };

    let y11 = if et < 0.0 && q == 0.0 && xi == 0.0 {
        0.0
    } else {
        1.0 / (r * ret.max(1e-12))
    };

    let y32 = if et < 0.0 && q == 0.0 && xi == 0.0 {
        0.0
    } else {
        (r + ret) * y11 * y11 / r.max(1e-12)
    };

    let r_safe = r.max(1e-12);
    let r3_safe = r3.max(1e-12);

    let ey = sd / r_safe - y * q / r3_safe;
    let ez = cd / r_safe + d * q / r3_safe;
    let fy = d / r3_safe + xi2 * y32 * sd;
    let fz = y / r3_safe + xi2 * y32 * cd;
    let gy = 2.0 * x11 * sd - y * q * x32;
    let gz = 2.0 * x11 * cd + d * q * x32;
    let hy = d * q * x32 + xi * q * y32 * sd;
    let hz = y * q * x32 + xi * q * y32 * cd;

    Dccon2 {
        r, r2, r3, r5, y, d, tt, alx, ale, x11, y11, x32, y32,
        ey, ez, fy, fz, gy, gz, hy, hz, xi2, et2, q2
    }
}

pub fn ua(xi: f64, et: f64, q: f64, disl1: f64, disl2: f64, disl3: f64, c0: &Dccon0, c2: &Dccon2) -> [f64; 12] {
    let pi2 = 2.0 * PI;
    let xy = xi * c2.y11;
    let qx = q * c2.x11;
    let qy = q * c2.y11;

    let mut u = [0.0; 12];

    if disl1 != 0.0 {
        let du = [
            c2.tt / 2.0 + c0.alp2 * xi * qy,
            c0.alp2 * q / c2.r,
            c0.alp1 * c2.ale - c0.alp2 * q * qy,
            -c0.alp1 * qy - c0.alp2 * c2.xi2 * q * c2.y32,
            -c0.alp2 * xi * q / c2.r3,
            c0.alp1 * xy + c0.alp2 * xi * c2.q2 * c2.y32,
            c0.alp1 * xy * c0.sd + c0.alp2 * xi * c2.fy + c2.d / 2.0 * c2.x11,
            c0.alp2 * c2.ey,
            c0.alp1 * (c0.cd / c2.r + qy * c0.sd) - c0.alp2 * q * c2.fy,
            c0.alp1 * xy * c0.cd + c0.alp2 * xi * c2.fz + c2.y / 2.0 * c2.x11,
            c0.alp2 * c2.ez,
            -c0.alp1 * (c0.sd / c2.r - qy * c0.cd) - c0.alp2 * q * c2.fz,
        ];
        for i in 0..12 { u[i] += (disl1 / pi2) * du[i]; }
    }

    if disl2 != 0.0 {
        let du = [
            c0.alp2 * q / c2.r,
            c2.tt / 2.0 + c0.alp2 * et * qx,
            c0.alp1 * c2.alx - c0.alp2 * q * qx,
            -c0.alp2 * xi * q / c2.r3,
            -qy / 2.0 - c0.alp2 * et * q / c2.r3,
            c0.alp1 / c2.r + c0.alp2 * c2.q2 / c2.r3,
            c0.alp2 * c2.ey,
            c0.alp1 * c2.d * c2.x11 + xy / 2.0 * c0.sd + c0.alp2 * et * c2.gy,
            c0.alp1 * c2.y * c2.x11 - c0.alp2 * q * c2.gy,
            c0.alp2 * c2.ez,
            c0.alp1 * c2.y * c2.x11 + xy / 2.0 * c0.cd + c0.alp2 * et * c2.gz,
            -c0.alp1 * c2.d * c2.x11 - c0.alp2 * q * c2.gz,
        ];
        for i in 0..12 { u[i] += (disl2 / pi2) * du[i]; }
    }

    if disl3 != 0.0 {
        let du = [
            -c0.alp1 * c2.ale - c0.alp2 * q * qy,
            -c0.alp1 * c2.alx - c0.alp2 * q * qx,
            c2.tt / 2.0 - c0.alp2 * (et * qx + xi * qy),
            -c0.alp1 * xy + c0.alp2 * xi * c2.q2 * c2.y32,
            -c0.alp1 / c2.r + c0.alp2 * c2.q2 / c2.r3,
            -c0.alp1 * qy - c0.alp2 * q * c2.q2 * c2.y32,
            -c0.alp1 * (c0.cd / c2.r + qy * c0.sd) - c0.alp2 * q * c2.fy,
            -c0.alp1 * c2.y * c2.x11 - c0.alp2 * q * c2.gy,
            c0.alp1 * (c2.d * c2.x11 + xy * c0.sd) + c0.alp2 * q * c2.hy,
            c0.alp1 * (c0.sd / c2.r - qy * c0.cd) - c0.alp2 * q * c2.fz,
            c0.alp1 * c2.d * c2.x11 - c0.alp2 * q * c2.gz,
            c0.alp1 * (c2.y * c2.x11 + xy * c0.cd) + c0.alp2 * q * c2.hz,
        ];
        for i in 0..12 { u[i] += (disl3 / pi2) * du[i]; }
    }

    u
}

pub fn ub(xi: f64, et: f64, q: f64, disl1: f64, disl2: f64, disl3: f64, c0: &Dccon0, c2: &Dccon2) -> [f64; 12] {
    let pi2 = 2.0 * PI;
    let rd = c2.r + c2.d;
    let rd2 = rd * rd;
    let d11 = 1.0 / (c2.r * rd.max(1e-12));
    let aj2 = xi * c2.y / rd.max(1e-12) * d11;
    let aj5 = -(c2.d + c2.y * c2.y / rd.max(1e-12)) * d11;

    let (cd, cdcd) = if c0.cd != 0.0 {
        (c0.cd, c0.cdcd)
    } else {
        (1e-12, 1e-12)
    };

    let x = (c2.xi2 + c2.q2).sqrt();

    let val1 = 1.0 / cdcd * (xi / rd.max(1e-12) * c0.sdcd + 2.0 * ((et * (x + q * cd) + x * (c2.r + x) * c0.sd) / (xi * (c2.r + x) * cd).max(1e-12)).atan());
    let val2 = xi * c2.y / (rd2 * 2.0).max(1e-12);
    let ai4 = if c0.cd != 0.0 {
        if xi == 0.0 { 0.0 } else { val1 }
    } else {
        val2
    };

    let ai3 = if c0.cd != 0.0 {
        (c2.y * cd / rd.max(1e-12) - c2.ale + c0.sd * rd.max(1e-12).ln()) / cdcd
    } else {
        (et / rd.max(1e-12) + c2.y * q / rd2.max(1e-12) - c2.ale) / 2.0
    };

    let ak1 = if c0.cd != 0.0 {
        xi * (d11 - c2.y11 * c0.sd) / cd
    } else {
        xi * q / rd.max(1e-12) * d11
    };

    let ak3 = if c0.cd != 0.0 {
        (q * c2.y11 - c2.y * d11) / cd
    } else {
        c0.sd / rd.max(1e-12) * (c2.xi2 * d11 - 1.0)
    };

    let aj3 = if c0.cd != 0.0 {
        (ak1 - aj2 * c0.sd) / cd
    } else {
        -xi / rd2.max(1e-12) * (c2.q2 * d11 - 0.5)
    };

    let aj6 = if c0.cd != 0.0 {
        (ak3 - aj5 * c0.sd) / cd
    } else {
        -c2.y / rd2.max(1e-12) * (c2.xi2 * d11 - 0.5)
    };

    let xy = xi * c2.y11;
    let ai1 = -xi / rd.max(1e-12) * cd - ai4 * c0.sd;
    let ai2 = rd.max(1e-12).ln() + ai3 * c0.sd;
    let ak2 = 1.0 / c2.r.max(1e-12) + ak3 * c0.sd;
    let ak4 = xy * cd - ak1 * c0.sd;
    let aj1 = aj5 * cd - aj6 * c0.sd;
    let aj4 = -xy - aj2 * cd + aj3 * c0.sd;

    let qx = q * c2.x11;
    let qy = q * c2.y11;

    let mut u = [0.0; 12];

    if disl1 != 0.0 {
        let du = [
            -xi * qy - c2.tt - c0.alp3 * ai1 * c0.sd,
            -q / c2.r.max(1e-12) + c0.alp3 * c2.y / rd.max(1e-12) * c0.sd,
            q * qy - c0.alp3 * ai2 * c0.sd,
            c2.xi2 * q * c2.y32 - c0.alp3 * aj1 * c0.sd,
            xi * q / c2.r3.max(1e-12) - c0.alp3 * aj2 * c0.sd,
            -xi * c2.q2 * c2.y32 - c0.alp3 * aj3 * c0.sd,
            -xi * c2.fy - c2.d * c2.x11 + c0.alp3 * (xy + aj4) * c0.sd,
            -c2.ey + c0.alp3 * (1.0 / c2.r.max(1e-12) + aj5) * c0.sd,
            q * c2.fy - c0.alp3 * (qy - aj6) * c0.sd,
            -xi * c2.fz - c2.y * c2.x11 + c0.alp3 * ak1 * c0.sd,
            -c2.ez + c0.alp3 * c2.y * d11 * c0.sd,
            q * c2.fz + c0.alp3 * ak2 * c0.sd,
        ];
        for i in 0..12 { u[i] += (disl1 / pi2) * du[i]; }
    }

    if disl2 != 0.0 {
        let du = [
            -q / c2.r.max(1e-12) + c0.alp3 * ai3 * c0.sdcd,
            -et * qx - c2.tt - c0.alp3 * xi / rd.max(1e-12) * c0.sdcd,
            q * qx + c0.alp3 * ai4 * c0.sdcd,
            xi * q / c2.r3.max(1e-12) + c0.alp3 * aj4 * c0.sdcd,
            et * q / c2.r3.max(1e-12) + qy + c0.alp3 * aj5 * c0.sdcd,
            -c2.q2 / c2.r3.max(1e-12) + c0.alp3 * aj6 * c0.sdcd,
            -c2.ey + c0.alp3 * aj1 * c0.sdcd,
            -et * c2.gy - xy * c0.sd + c0.alp3 * aj2 * c0.sdcd,
            q * c2.gy + c0.alp3 * aj3 * c0.sdcd,
            -c2.ez - c0.alp3 * ak3 * c0.sdcd,
            -et * c2.gz - xy * cd - c0.alp3 * xi * d11 * c0.sdcd,
            q * c2.gz - c0.alp3 * ak4 * c0.sdcd,
        ];
        for i in 0..12 { u[i] += (disl2 / pi2) * du[i]; }
    }

    if disl3 != 0.0 {
        let du = [
            q * qy - c0.alp3 * ai3 * c0.sdsd,
            q * qx + c0.alp3 * xi / rd.max(1e-12) * c0.sdsd,
            et * qx + xi * qy - c2.tt - c0.alp3 * ai4 * c0.sdsd,
            -xi * c2.q2 * c2.y32 - c0.alp3 * aj4 * c0.sdsd,
            -c2.q2 / c2.r3.max(1e-12) - c0.alp3 * aj5 * c0.sdsd,
            q * c2.q2 * c2.y32 - c0.alp3 * aj6 * c0.sdsd,
            q * c2.fy - c0.alp3 * aj1 * c0.sdsd,
            q * c2.gy - c0.alp3 * aj2 * c0.sdsd,
            -q * c2.hy - c0.alp3 * aj3 * c0.sdsd,
            q * c2.fz + c0.alp3 * ak3 * c0.sdsd,
            q * c2.gz + c0.alp3 * xi * d11 * c0.sdsd,
            -q * c2.hz + c0.alp3 * ak4 * c0.sdsd,
        ];
        for i in 0..12 { u[i] += (disl3 / pi2) * du[i]; }
    }

    u
}

pub fn uc(xi: f64, et: f64, q: f64, z: f64, disl1: f64, disl2: f64, disl3: f64, c0: &Dccon0, c2: &Dccon2) -> [f64; 12] {
    let pi2 = 2.0 * PI;
    let c = c2.d + z;
    
    let r2 = c2.r2.max(1e-12);
    let r3 = c2.r3.max(1e-12);
    let r5 = c2.r5.max(1e-12);
    
    let x53 = (8.0 * c2.r2 + 9.0 * c2.r * xi + 3.0 * c2.xi2) * c2.x11.powi(3) / r2;
    let y53 = (8.0 * c2.r2 + 9.0 * c2.r * et + 3.0 * c2.et2) * c2.y11.powi(3) / r2;
    
    let h = q * c0.cd - z;
    let z32 = c0.sd / r3 - h * c2.y32;
    let z53 = 3.0 * c0.sd / r5 - h * y53;
    let y0 = c2.y11 - c2.xi2 * c2.y32;
    let z0 = z32 - c2.xi2 * z53;
    let ppy = c0.cd / r3 + q * c2.y32 * c0.sd;
    let ppz = c0.sd / r3 - q * c2.y32 * c0.cd;
    let qq = z * c2.y32 + z32 + z0;
    let qqy = 3.0 * c * c2.d / r5 - qq * c0.sd;
    let qqz = 3.0 * c * c2.y / r5 - qq * c0.cd + q * c2.y32;
    let xy = xi * c2.y11;
    let qy = q * c2.y11;
    let qr = 3.0 * q / r5;
    let cdr = (c + c2.d) / r3;
    let yy0 = c2.y / r3 - y0 * c0.cd;

    let mut u = [0.0; 12];

    if disl1 != 0.0 {
        let du = [
            c0.alp4 * xy * c0.cd - c0.alp5 * xi * q * z32,
            c0.alp4 * (c0.cd / c2.r.max(1e-12) + 2.0 * qy * c0.sd) - c0.alp5 * c * q / r3,
            c0.alp4 * qy * c0.cd - c0.alp5 * (c * et / r3 - z * c2.y11 + c2.xi2 * z32),
            c0.alp4 * y0 * c0.cd - c0.alp5 * q * z0,
            -c0.alp4 * xi * (c0.cd / r3 + 2.0 * q * c2.y32 * c0.sd) + c0.alp5 * c * xi * qr,
            -c0.alp4 * xi * q * c2.y32 * c0.cd + c0.alp5 * xi * (3.0 * c * et / r5 - qq),
            -c0.alp4 * xi * ppy * c0.cd - c0.alp5 * xi * qqy,
            c0.alp4 * 2.0 * (c2.d / r3 - y0 * c0.sd) * c0.sd - c2.y / r3 * c0.cd - c0.alp5 * (cdr * c0.sd - et / r3 - c * c2.y * qr),
            -c0.alp4 * q / r3 + yy0 * c0.sd + c0.alp5 * (cdr * c0.cd + c * c2.d * qr - (y0 * c0.cd + q * z0) * c0.sd),
            c0.alp4 * xi * ppz * c0.cd - c0.alp5 * xi * qqz,
            c0.alp4 * 2.0 * (c2.y / r3 - y0 * c0.cd) * c0.sd + c2.d / r3 * c0.cd - c0.alp5 * (cdr * c0.cd + c * c2.d * qr),
            yy0 * c0.cd - c0.alp5 * (cdr * c0.sd - c * c2.y * qr - y0 * c0.sdsd + q * z0 * c0.cd),
        ];
        for i in 0..12 { u[i] += (disl1 / pi2) * du[i]; }
    }

    if disl2 != 0.0 {
        let du = [
            c0.alp4 * c0.cd / c2.r.max(1e-12) - qy * c0.sd - c0.alp5 * c * q / r3,
            c0.alp4 * c2.y * c2.x11 - c0.alp5 * c * et * q * c2.x32,
            -c2.d * c2.x11 - xy * c0.sd - c0.alp5 * c * (c2.x11 - c2.q2 * c2.x32),
            -c0.alp4 * xi / r3 * c0.cd + c0.alp5 * c * xi * qr + xi * q * c2.y32 * c0.sd,
            -c0.alp4 * c2.y / r3 + c0.alp5 * c * et * qr,
            c2.d / r3 - y0 * c0.sd + c0.alp5 * c / r3 * (1.0 - 3.0 * c2.q2 / r2),
            -c0.alp4 * et / r3 + y0 * c0.sdsd - c0.alp5 * (cdr * c0.sd - c * c2.y * qr),
            c0.alp4 * (c2.x11 - c2.y * c2.y * c2.x32) - c0.alp5 * c * ((c2.d + 2.0 * q * c0.cd) * c2.x32 - c2.y * et * q * x53),
            xi * ppy * c0.sd + c2.y * c2.d * c2.x32 + c0.alp5 * c * ((c2.y + 2.0 * q * c0.sd) * c2.x32 - c2.y * c2.q2 * x53),
            -q / r3 + y0 * c0.sdcd - c0.alp5 * (cdr * c0.cd + c * c2.d * qr),
            c0.alp4 * c2.y * c2.d * c2.x32 - c0.alp5 * c * ((c2.y - 2.0 * q * c0.sd) * c2.x32 + c2.d * et * q * x53),
            -xi * ppz * c0.sd + c2.x11 - c2.d * c2.d * c2.x32 - c0.alp5 * c * ((c2.d - 2.0 * q * c0.cd) * c2.x32 - c2.d * c2.q2 * x53),
        ];
        for i in 0..12 { u[i] += (disl2 / pi2) * du[i]; }
    }

    if disl3 != 0.0 {
        let du = [
            -c0.alp4 * (c0.sd / c2.r.max(1e-12) + qy * c0.cd) - c0.alp5 * (z * c2.y11 - c2.q2 * z32),
            c0.alp4 * 2.0 * xy * c0.sd + c2.d * c2.x11 - c0.alp5 * c * (c2.x11 - c2.q2 * c2.x32),
            c0.alp4 * (c2.y * c2.x11 + xy * c0.cd) + c0.alp5 * q * (c * et * c2.x32 + xi * z32),
            c0.alp4 * xi / r3 * c0.sd + xi * q * c2.y32 * c0.cd + c0.alp5 * xi * (3.0 * c * et / r5 - 2.0 * z32 - z0),
            c0.alp4 * 2.0 * y0 * c0.sd - c2.d / r3 + c0.alp5 * c / r3 * (1.0 - 3.0 * c2.q2 / r2),
            -c0.alp4 * yy0 - c0.alp5 * (c * et * qr - q * z0),
            c0.alp4 * (q / r3 + y0 * c0.sdcd) + c0.alp5 * (z / r3 * c0.cd + c * c2.d * qr - q * z0 * c0.sd),
            -c0.alp4 * 2.0 * xi * ppy * c0.sd - c2.y * c2.d * c2.x32 + c0.alp5 * c * ((c2.y + 2.0 * q * c0.sd) * c2.x32 - c2.y * c2.q2 * x53),
            -c0.alp4 * (xi * ppy * c0.cd - c2.x11 + c2.y * c2.y * c2.x32) + c0.alp5 * (c * ((c2.d + 2.0 * q * c0.cd) * c2.x32 - c2.y * et * q * x53) + xi * qqy),
            -et / r3 + y0 * c0.cdcd - c0.alp5 * (z / r3 * c0.sd - c * c2.y * qr - y0 * c0.sdsd + q * z0 * c0.cd),
            c0.alp4 * 2.0 * xi * ppz * c0.sd - c2.x11 + c2.d * c2.d * c2.x32 - c0.alp5 * c * ((c2.d - 2.0 * q * c0.cd) * c2.x32 - c2.d * c2.q2 * x53),
            c0.alp4 * (xi * ppz * c0.cd + c2.y * c2.d * c2.x32) + c0.alp5 * (c * ((c2.y - 2.0 * q * c0.sd) * c2.x32 + c2.d * et * q * x53) + xi * qqz),
        ];
        for i in 0..12 { u[i] += (disl3 / pi2) * du[i]; }
    }

    u
}

pub fn okada_dc3d_single(alpha: f64, x: f64, y: f64, z: f64, depth: f64, dip: f64, al1: f64, al2: f64, aw1: f64, aw2: f64, disl1: f64, disl2: f64, disl3: f64) -> ([f64; 12], bool) {
    let mut u_out = [0.0; 12];
    let mut iret = false;
    
    let c0 = dccon0(alpha, dip);
    
    let d = depth + z;
    let p = y * c0.cd + d * c0.sd;
    let q = y * c0.sd - d * c0.cd;
    
    let jxi = if (x + al1) * (x - al2) <= 0.0 { 1 } else { 0 };
    let jet = if (p + aw1) * (p - aw2) <= 0.0 { 1 } else { 0 };
    
    for k in 1..=2 {
        let et = if k == 1 { p + aw1 } else { p - aw2 };
        for j in 1..=2 {
            let xi = if j == 1 { x + al1 } else { x - al2 };
            
            let c2 = dccon2(xi, et, q, c0.sd, c0.cd);
            
            if (jxi == 1 && q.abs() <= 1e-12 && et.abs() <= 1e-12) || (jet == 1 && q.abs() <= 1e-12 && xi.abs() <= 1e-12) {
                iret = true;
            }
            
            let dua = ua(xi, et, q, disl1, disl2, disl3, &c0, &c2);
            
            let mut du = [0.0; 12];
            for i in (0..12).step_by(3) {
                du[i] = -dua[i];
                du[i+1] = -dua[i+1] * c0.cd + dua[i+2] * c0.sd;
                du[i+2] = -dua[i+1] * c0.sd - dua[i+2] * c0.cd;
            }
            
            du[9] = -du[9];
            du[10] = -du[10];
            du[11] = -du[11];
            
            if (j + k) != 3 {
                for i in 0..12 { u_out[i] += du[i]; }
            } else {
                for i in 0..12 { u_out[i] -= du[i]; }
            }
        }
    }
    
    let zz = z;
    let d_im = depth - z;
    let p_im = y * c0.cd + d_im * c0.sd;
    let q_im = y * c0.sd - d_im * c0.cd;
    
    for k in 1..=2 {
        let et = if k == 1 { p_im + aw1 } else { p_im - aw2 };
        for j in 1..=2 {
            let xi = if j == 1 { x + al1 } else { x - al2 };
            
            let c2_im = dccon2(xi, et, q_im, c0.sd, c0.cd);
            
            let dua = ua(xi, et, q_im, disl1, disl2, disl3, &c0, &c2_im);
            let dub = ub(xi, et, q_im, disl1, disl2, disl3, &c0, &c2_im);
            let duc = uc(xi, et, q_im, zz, disl1, disl2, disl3, &c0, &c2_im);
            
            let mut du = [0.0; 12];
            for i in (0..12).step_by(3) {
                du[i] = dua[i] + dub[i] + zz * duc[i];
                du[i+1] = (dua[i+1] + dub[i+1] + zz * duc[i+1]) * c0.cd - (dua[i+2] + dub[i+2] + zz * duc[i+2]) * c0.sd;
                du[i+2] = (dua[i+1] + dub[i+1] - zz * duc[i+1]) * c0.sd + (dua[i+2] + dub[i+2] - zz * duc[i+2]) * c0.cd;
            }
            
            du[9] += duc[0];
            du[10] += duc[1] * c0.cd - duc[2] * c0.sd;
            du[11] -= duc[1] * c0.sd + duc[2] * c0.cd;
            
            if (j + k) != 3 {
                for i in 0..12 { u_out[i] += du[i]; }
            } else {
                for i in 0..12 { u_out[i] -= du[i]; }
            }
        }
    }
    
    (u_out, iret)
}

pub fn okada_dc3d0_single(alpha: f64, x: f64, y: f64, z: f64, depth: f64, dip: f64, pot1: f64, pot2: f64, pot3: f64, pot4: f64) -> ([f64; 12], bool) {
    ([0.0; 12], false) // Point source wrapper omitted for now as in Python
}
