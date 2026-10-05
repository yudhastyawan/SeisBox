use std::fmt;

/// Seismic phase types supported for picking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhaseType {
    /// P-wave first arrival (onset).
    PStart,
    /// S-wave first arrival (onset).
    SStart,
    /// P-wave end / coda end.
    PEnd,
    /// S-wave end / coda end.
    SEnd,
}

impl PhaseType {
    /// Label for display on the plot.
    pub fn label(&self) -> &'static str {
        match self {
            PhaseType::PStart => "P",
            PhaseType::SStart => "S",
            PhaseType::PEnd => "P-end",
            PhaseType::SEnd => "S-end",
        }
    }

    /// Colour for the pick marker (egui Color32).
    pub fn color(&self) -> eframe::egui::Color32 {
        match self {
            PhaseType::PStart => eframe::egui::Color32::from_rgb(255, 80, 80),   // Red
            PhaseType::SStart => eframe::egui::Color32::from_rgb(80, 140, 255),  // Blue
            PhaseType::PEnd => eframe::egui::Color32::from_rgb(255, 180, 50),    // Orange
            PhaseType::SEnd => eframe::egui::Color32::from_rgb(50, 220, 220),    // Cyan
        }
    }

    /// Serialisation tag used in the ASCII pick file.
    pub fn tag(&self) -> &'static str {
        match self {
            PhaseType::PStart => "P_START",
            PhaseType::SStart => "S_START",
            PhaseType::PEnd => "P_END",
            PhaseType::SEnd => "S_END",
        }
    }

    /// Parse from an ASCII pick file tag.
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "P_START" => Some(PhaseType::PStart),
            "S_START" => Some(PhaseType::SStart),
            "P_END" => Some(PhaseType::PEnd),
            "S_END" => Some(PhaseType::SEnd),
            _ => None,
        }
    }

    /// All phase types in picking order.
    pub fn all() -> &'static [PhaseType] {
        &[
            PhaseType::PStart,
            PhaseType::SStart,
            PhaseType::PEnd,
            PhaseType::SEnd,
        ]
    }
}

impl fmt::Display for PhaseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Onset {
    Impulsive,
    Emergent,
}

impl Onset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Onset::Impulsive => "i",
            Onset::Emergent => "e",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "i" | "I" => Some(Onset::Impulsive),
            "e" | "E" => Some(Onset::Emergent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    Up,
    Down,
}

impl Polarity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Polarity::Up => "U",
            Polarity::Down => "D",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "u" | "U" | "+" => Some(Polarity::Up),
            "d" | "D" | "-" => Some(Polarity::Down),
            _ => None,
        }
    }
}

/// A single phase pick at a specific time.
#[derive(Debug, Clone)]
pub struct Pick {
    pub phase: PhaseType,
    /// Time in seconds (X-axis value on the seismogram plot).
    pub time: f64,
    pub onset: Option<Onset>,
    pub polarity: Option<Polarity>,
    pub uncertainty: Option<f64>,
    pub amplitude: Option<f64>,
    pub amplitude_demeaned: Option<f64>,
}

/// Container for all picks on a single trace.
///
/// Enforces at most one pick per `PhaseType`.
#[derive(Debug, Clone, Default)]
pub struct PickSet {
    pub picks: Vec<Pick>,
}

impl PickSet {
    pub fn new() -> Self {
        Self { picks: Vec::new() }
    }

    /// Add or update a pick for the given phase. Returns true if a new pick was added,
    /// false if an existing one was updated.
    pub fn add_or_update(&mut self, phase: PhaseType, time: f64) -> bool {
        if let Some(existing) = self.picks.iter_mut().find(|p| p.phase == phase) {
            existing.time = time;
            false
        } else {
            self.picks.push(Pick {
                phase,
                time,
                onset: None,
                polarity: None,
                uncertainty: None,
                amplitude: None,
                amplitude_demeaned: None,
            });
            true
        }
    }

    /// Delete a pick for the given phase. Returns true if a pick was removed.
    pub fn delete(&mut self, phase: PhaseType) -> bool {
        let before = self.picks.len();
        self.picks.retain(|p| p.phase != phase);
        self.picks.len() < before
    }

    /// Remove all picks.
    pub fn wipe(&mut self) {
        self.picks.clear();
    }

    /// Get the time value for a specific phase, if picked.
    pub fn get(&self, phase: PhaseType) -> Option<f64> {
        self.picks
            .iter()
            .find(|p| p.phase == phase)
            .map(|p| p.time)
    }

    /// Check if any picks exist.
    pub fn is_empty(&self) -> bool {
        self.picks.is_empty()
    }

    /// Number of picks.
    pub fn len(&self) -> usize {
        self.picks.len()
    }
}
