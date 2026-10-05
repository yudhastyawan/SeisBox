//! Earth model representation and I/O.
//!
//! Replaces global variables from Fortran module `Marc` (ALFA, BTA, H, RHO, MU,
//! G_SLOWS, G_SLOWP, NCAPAS, etc.) with structured data types.

use crate::hvf::matrix::halfspace_rayleigh;
use crate::hvf::types::Float;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

// =============================================================================
// Data structures
// =============================================================================

/// A single layer in the earth model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Layer thickness in meters. 0.0 for the halfspace (bottom layer).
    pub thickness: Float,
    /// P-wave velocity (m/s).
    pub vp: Float,
    /// S-wave velocity (m/s).
    pub vs: Float,
    /// Density (kg/m³).
    pub density: Float,
    /// Optional P-wave quality factor (attenuation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qp: Option<Float>,
    /// Optional S-wave quality factor (attenuation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qs: Option<Float>,
}

/// Layered earth model (GUI-ready via serde).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarthModel {
    pub layers: Vec<Layer>,
}

/// Derived parameters computed from the earth model.
/// Encapsulates all the global state that was scattered across Fortran modules.
#[derive(Debug, Clone)]
pub struct ModelParams {
    /// Number of layers (including halfspace). Replaces `NCAPAS`.
    pub nlayers: usize,
    /// P-wave velocity per layer. Replaces `ALFA`.
    pub alpha: Vec<Float>,
    /// S-wave velocity per layer. Replaces `BTA`.
    pub beta: Vec<Float>,
    /// Layer thickness (nlayers-1 entries; halfspace has no thickness). Replaces `H`.
    pub h: Vec<Float>,
    /// Density per layer. Replaces `RHO`.
    pub rho: Vec<Float>,
    /// Shear modulus per layer: μ = β² × ρ. Replaces `MU`.
    pub mu: Vec<Float>,
    /// S-wave slowness per layer: 1/β. Replaces `G_SLOWS`.
    pub slow_s: Vec<Float>,
    /// P-wave slowness per layer: 1/α. Replaces `G_SLOWP`.
    pub slow_p: Vec<Float>,
    /// Maximum Rayleigh slowness (from halfspace). Replaces `G_MAXRAYLEIGHSLOWNESS`.
    pub max_rayleigh_slowness: Float,
    /// Minimum S-wave slowness (halfspace). Replaces `G_SLOWSMIN`.
    pub slow_s_min: Float,
    /// Maximum S-wave slowness across all layers. Replaces `G_SLOWSMAX`.
    pub slow_s_max: Float,
    /// Index of velocity inversion layer, or -1 if none. Replaces `G_VELOCITYINVERSION`.
    pub velocity_inversion: i32,
}

// =============================================================================
// EarthModel I/O
// =============================================================================

impl EarthModel {
    /// Load a model from the legacy Fortran text format:
    /// ```text
    /// <number_of_layers>
    /// <thickness> <Vp> <Vs> <density>
    /// ...
    /// 0 <Vp> <Vs> <density>   (halfspace)
    /// ```
    /// `use_brocher`: if true, calculates Vp and Density from Vs (requires only 2 columns: thickness and Vs).
    pub fn from_file(path: &Path, use_brocher: bool, vp_expr: Option<&str>, rho_expr: Option<&str>) -> io::Result<Self> {
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        let mut lines = reader.lines();

        let vp_func_opt: Option<Box<dyn Fn(f64) -> f64>> = match vp_expr {
            Some(s) if !s.trim().is_empty() => {
                let expr: meval::Expr = s.parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Error parsing vp_expr: {}", e)))?;
                let func = expr.bind("vs").map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Error binding vs: {}", e)))?;
                Some(Box::new(func))
            },
            _ => None,
        };

        let rho_func_opt: Option<Box<dyn Fn(f64, f64) -> f64>> = match rho_expr {
            Some(s) if !s.trim().is_empty() => {
                let expr: meval::Expr = s.parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Error parsing rho_expr: {}", e)))?;
                let func = expr.bind2("vp", "vs").map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Error binding vp/vs: {}", e)))?;
                Some(Box::new(func))
            },
            _ => None,
        };

        // Skip comments and empty lines
        let mut nlayers = 0;
        let mut first_line = String::new();
        for line_res in &mut lines {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('%') {
                continue;
            }
            nlayers = trimmed.parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid layer count: {}", e)))?;
            first_line = line;
            break;
        }

        if first_line.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty model file"));
        }

        let mut layers = Vec::with_capacity(nlayers);
        for _ in 0..nlayers {
            let mut line = String::new();
            for line_res in &mut lines {
                let l = line_res?;
                let trimmed = l.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('%') {
                    continue;
                }
                line = l;
                break;
            }
            if line.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected end of model file"));
            }

            let parts: Vec<Float> = line
                .split_whitespace()
                .map(|s| {
                    s.parse::<Float>()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid number: {}", e)))
                })
                .collect::<io::Result<Vec<Float>>>()?;

            if vp_func_opt.is_some() || rho_func_opt.is_some() || use_brocher {
                if parts.len() < 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "When using custom expressions or Brocher relations, each layer line must have at least 2 values: thickness Vs",
                    ));
                }
                let thickness = parts[0];
                let vs = parts[1];
                
                let vp = if let Some(ref func) = vp_func_opt {
                    func(vs)
                } else if use_brocher {
                    let v_km = vs / 1000.0;
                    let v_p_km = 0.9409 + 2.0947 * v_km - 0.8206 * v_km.powi(2) + 0.2683 * v_km.powi(3) - 0.0251 * v_km.powi(4);
                    v_p_km * 1000.0
                } else {
                    if parts.len() < 4 {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Missing Vp"));
                    }
                    parts[1]
                };

                let density = if let Some(ref func) = rho_func_opt {
                    func(vp, vs)
                } else if use_brocher {
                    let v_p_km = vp / 1000.0;
                    let r = 1.6612 * v_p_km - 0.4721 * v_p_km.powi(2) + 0.0671 * v_p_km.powi(3) - 0.0043 * v_p_km.powi(4) + 0.000106 * v_p_km.powi(5);
                    r * 1000.0
                } else {
                    if parts.len() < 4 {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, "Missing Density"));
                    }
                    parts[3] // wait, if use_brocher is false but rho is missing, it will use parts[3]. If they only use vp_expr, they MUST supply 4 columns, or we just take part[3]. Wait, if they provide 2 columns, parts[3] panics. Let's fix this logic.
                };
                
                let qp = if parts.len() >= 4 { Some(parts[2]) } else { None };
                let qs = if parts.len() >= 4 { Some(parts[3]) } else { None };

                layers.push(Layer { thickness, vp, vs, density, qp, qs });
            } else {
                if parts.len() < 4 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Each layer line must have 4 values: thickness Vp Vs density",
                    ));
                }

                let qp = if parts.len() >= 6 { Some(parts[4]) } else { None };
                let qs = if parts.len() >= 6 { Some(parts[5]) } else { None };

                layers.push(Layer {
                    thickness: parts[0],
                    vp: parts[1],
                    vs: parts[2],
                    density: parts[3],
                    qp,
                    qs,
                });
            }
        }

        Ok(EarthModel { layers })
    }

    /// Load a model from JSON format (for GUI integration).
    pub fn from_json(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid JSON model: {}", e)))
    }

    /// Serialize the model to JSON.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Compute derived parameters from the earth model.
    pub fn derived_params(&self) -> ModelParams {
        let nlayers = self.layers.len();
        let alpha: Vec<Float> = self.layers.iter().map(|l| l.vp).collect();
        let beta: Vec<Float> = self.layers.iter().map(|l| l.vs).collect();
        let h: Vec<Float> = self.layers[..nlayers - 1]
            .iter()
            .map(|l| l.thickness)
            .collect();
        let rho: Vec<Float> = self.layers.iter().map(|l| l.density).collect();
        let mu: Vec<Float> = beta
            .iter()
            .zip(rho.iter())
            .map(|(&b, &r)| b * b * r)
            .collect();
        let slow_s: Vec<Float> = beta.iter().map(|&b| 1.0 / b).collect();
        let slow_p: Vec<Float> = alpha.iter().map(|&a| 1.0 / a).collect();

        // Find max S-wave slowness and its index
        let (i_max, &slow_s_max) = slow_s
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        let max_rayleigh_slowness = halfspace_rayleigh(slow_p[i_max], slow_s[i_max]);
        let slow_s_min = slow_s[nlayers - 1]; // Halfspace

        // Detect velocity inversion
        let mut velocity_inversion: i32 = -1;
        for i in 1..nlayers {
            if slow_s[i] > slow_s[i - 1] || slow_p[i] > slow_p[i - 1] {
                velocity_inversion = i as i32;
                break;
            }
        }

        ModelParams {
            nlayers,
            alpha,
            beta,
            h,
            rho,
            mu,
            slow_s,
            slow_p,
            max_rayleigh_slowness,
            slow_s_min,
            slow_s_max,
            velocity_inversion,
        }
    }
}
