//! Output formatting (text and JSON).

use serde::{Serialize, Deserialize};

use crate::hvf::types::Float;

/// Structured output for JSON mode (GUI integration).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HvsrOutput {
    /// Frequencies in Hz.
    pub frequencies: Vec<Float>,
    /// H/V spectral ratio at each frequency.
    pub hv_ratio: Vec<Float>,
    /// Rayleigh phase slowness per mode (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rayleigh_phase: Option<DispersionOutput>,
    /// Love phase slowness per mode (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub love_phase: Option<DispersionOutput>,
}

/// Dispersion curve output for a single wave type.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DispersionOutput {
    pub num_frequencies: usize,
    pub num_modes: usize,
    /// Slowness values: `[mode][freq]`.
    pub slowness: Vec<Vec<Float>>,
    /// Validity flags.
    pub valid: Vec<Vec<bool>>,
}

impl HvsrOutput {
    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
    }
}
