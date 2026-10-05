use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use chrono::{Datelike, Timelike, NaiveDateTime};

use crate::core::seismogram::Seismogram;

const SAC_FLOAT_UNDEF: f32 = -12345.0;
const SAC_INT_UNDEF: i32 = -12345;
const _SAC_STRING_UNDEF: &str = "-12345  ";

/// Export a seismogram trace to SAC binary format (Little Endian).
pub fn export_seismogram_to_sac(seis: &Seismogram, path: &Path) -> io::Result<()> {
    let mut file = File::create(path)?;

    let npts = seis.time.len() as i32;
    let delta = if seis.sample_rate > 0.0 { 1.0 / seis.sample_rate as f32 } else { 0.0 };
    let b = 0.0f32; // relative to start time
    let e = if npts > 0 { b + (npts - 1) as f32 * delta } else { 0.0 };
    
    let depmin = seis.amplitude.iter().map(|&a| a as f32).fold(f32::INFINITY, f32::min);
    let depmax = seis.amplitude.iter().map(|&a| a as f32).fold(f32::NEG_INFINITY, f32::max);
    let depmen = if npts > 0 {
        seis.amplitude.iter().map(|&a| a as f32).sum::<f32>() / npts as f32
    } else {
        0.0
    };

    // Header floats (70 elements)
    let mut fhdr = [SAC_FLOAT_UNDEF; 70];
    fhdr[0] = delta;
    fhdr[1] = depmin;
    fhdr[2] = depmax;
    fhdr[5] = b;
    fhdr[6] = e;
    fhdr[56] = depmen;

    // Header ints (40 elements)
    let mut ihdr = [SAC_INT_UNDEF; 40];
    
    // Parse start time to get nzyear, nzjday, nzhour, nzmin, nzsec, nzmsec
    // Trying generic ISO 8601 parsing or similar. The seismogram start_time_str usually looks like "YYYY-MM-DDTHH:MM:SS.fffZ"
    let time_str = seis.start_time_str.replace("Z", "").replace("T", " ");
    if let Ok(st) = NaiveDateTime::parse_from_str(&time_str, "%Y-%m-%d %H:%M:%S%.f") {
        ihdr[0] = st.year() as i32;
        ihdr[1] = st.ordinal() as i32;
        ihdr[2] = st.hour() as i32;
        ihdr[3] = st.minute() as i32;
        ihdr[4] = st.second() as i32;
        ihdr[5] = (st.nanosecond() / 1_000_000) as i32;
    } else if let Ok(st) = NaiveDateTime::parse_from_str(&time_str, "%Y-%m-%d %H:%M:%S") {
        ihdr[0] = st.year() as i32;
        ihdr[1] = st.ordinal() as i32;
        ihdr[2] = st.hour() as i32;
        ihdr[3] = st.minute() as i32;
        ihdr[4] = st.second() as i32;
        ihdr[5] = 0;
    }

    ihdr[6] = 6; // nvhdr
    ihdr[9] = npts; // npts
    ihdr[15] = 1; // iftype (ITIME)
    ihdr[35] = 1; // leven (TRUE)
    ihdr[36] = 1; // lpspol (TRUE)
    ihdr[37] = 1; // lovrok (TRUE)
    ihdr[38] = 1; // lcalda (TRUE)

    for val in &fhdr {
        file.write_all(&val.to_le_bytes())?;
    }
    for val in &ihdr {
        file.write_all(&val.to_le_bytes())?;
    }

    // Header strings (24 elements of 8 bytes)
    // 0: kstnm, 1: kevnm(16), 3: khole, 4: ko, 5: ka, ..., 20: kcmpnm, 21: knetwk
    let mut shdr = vec![b' '; 24 * 8];
    
    let mut write_str = |idx: usize, s: &str| {
        let bytes = s.as_bytes();
        let copy_len = bytes.len().min(8);
        for i in 0..copy_len {
            shdr[idx * 8 + i] = bytes[i];
        }
    };

    write_str(0, &seis.station);
    write_str(20, &seis.channel);
    write_str(21, &seis.network);

    file.write_all(&shdr)?;

    // Data
    for &amp in &seis.amplitude {
        file.write_all(&(amp as f32).to_le_bytes())?;
    }

    Ok(())
}
