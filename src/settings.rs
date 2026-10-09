use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    /// Relative to the original camera speed; 0.5 halves its sensitivity.
    pub mouse_sensitivity: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.5,
        }
    }
}
impl Settings {
    pub fn radians_per_pixel(&self) -> f32 {
        0.006 * self.mouse_sensitivity
    }
    pub fn load() -> Self {
        let Ok(bytes) = std::fs::read("saves/settings.json") else {
            return Self::default();
        };
        let Ok(settings) = serde_json::from_slice::<Self>(&bytes) else {
            return Self::default();
        };
        if !settings.mouse_sensitivity.is_finite() {
            return Self::default();
        }
        Self {
            mouse_sensitivity: settings.mouse_sensitivity.clamp(0.1, 2.0),
        }
    }
    pub fn save(&self) -> Result<(), String> {
        let path = Path::new("saves/settings.json");
        std::fs::create_dir_all("saves").map_err(|e| e.to_string())?;
        std::fs::write(path, serde_json::to_vec_pretty(self).unwrap()).map_err(|e| e.to_string())
    }
}
