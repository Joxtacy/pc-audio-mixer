use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotentiometerData {
    pub pot1: u16,
    pub pot2: u16,
    pub pot3: u16,
}

impl PotentiometerData {
    pub fn to_percentages(&self) -> (f32, f32, f32) {
        // Helper function to round to nearest 2%
        let round_to_2 = |val: f32| -> f32 {
            let percentage = (val / 4095.0) * 100.0;
            (percentage / 2.0).round() * 2.0
        };

        (
            round_to_2(self.pot1 as f32),
            round_to_2(self.pot2 as f32),
            round_to_2(self.pot3 as f32),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerChannel {
    pub id: usize,
    pub value: f32, // 0.0 to 100.0
    pub is_physical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSession {
    pub process_id: u32,
    pub process_name: String,
    pub display_name: String,
    pub volume: f32, // 0.0 to 100.0
    pub is_muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialPortInfo {
    pub port_name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub port: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub auto_connect: bool,
    pub theme: String,
}
