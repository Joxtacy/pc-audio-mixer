use anyhow::{anyhow, Result};
use serde_json;
use serialport::{self, SerialPort};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::types::{ConnectionStatus, PotentiometerData, SerialPortInfo};

pub struct SerialManager {
    port: Arc<Mutex<Option<Box<dyn SerialPort>>>>,
    port_name: Arc<Mutex<Option<String>>>,
}

impl SerialManager {
    pub fn new() -> Self {
        Self {
            port: Arc::new(Mutex::new(None)),
            port_name: Arc::new(Mutex::new(None)),
        }
    }

    pub fn list_ports() -> Result<Vec<SerialPortInfo>> {
        let ports = serialport::available_ports()
            .map_err(|e| anyhow!("Failed to list ports: {}", e))?;

        Ok(ports
            .into_iter()
            .map(|p| SerialPortInfo {
                port_name: p.port_name.clone(),
                description: match p.port_type {
                    serialport::SerialPortType::UsbPort(info) => {
                        format!(
                            "{} - {}",
                            info.product.unwrap_or_else(|| "Unknown".to_string()),
                            info.manufacturer.unwrap_or_else(|| "Unknown".to_string())
                        )
                    }
                    _ => "Serial Port".to_string(),
                },
            })
            .collect())
    }

    pub fn find_pico_port() -> Option<String> {
        if let Ok(ports) = serialport::available_ports() {
            for port in ports {
                let port_name_lower = port.port_name.to_lowercase();

                // Check for Pico identifiers
                if let serialport::SerialPortType::UsbPort(info) = &port.port_type {
                    if let Some(product) = &info.product {
                        let product_lower = product.to_lowercase();
                        if product_lower.contains("pico") || product_lower.contains("rp2040") {
                            return Some(port.port_name);
                        }
                    }

                    if let Some(manufacturer) = &info.manufacturer {
                        let manufacturer_lower = manufacturer.to_lowercase();
                        if manufacturer_lower.contains("raspberry") {
                            return Some(port.port_name);
                        }
                    }
                }

                // Check for common patterns
                if port_name_lower.contains("usbmodem") ||
                   port_name_lower.contains("ttyacm") ||
                   port_name_lower.contains("com") && port_name_lower.len() <= 5 {
                    // This might be our device
                    return Some(port.port_name);
                }
            }
        }
        None
    }

    pub fn connect(&self, port_name: Option<String>) -> Result<ConnectionStatus> {
        // Disconnect if already connected
        self.disconnect();

        let port_to_use = port_name.or_else(Self::find_pico_port);

        if let Some(port_name) = port_to_use {
            match serialport::new(&port_name, 115200)
                .timeout(Duration::from_millis(1000))
                .open()
            {
                Ok(port) => {
                    *self.port.lock().unwrap() = Some(port);
                    *self.port_name.lock().unwrap() = Some(port_name.clone());

                    Ok(ConnectionStatus {
                        connected: true,
                        port: Some(port_name),
                        error: None,
                    })
                }
                Err(e) => Ok(ConnectionStatus {
                    connected: false,
                    port: None,
                    error: Some(format!("Failed to connect: {}", e)),
                }),
            }
        } else {
            Ok(ConnectionStatus {
                connected: false,
                port: None,
                error: Some("No Pico device found".to_string()),
            })
        }
    }

    pub fn disconnect(&self) {
        *self.port.lock().unwrap() = None;
        *self.port_name.lock().unwrap() = None;
    }

    pub fn is_connected(&self) -> bool {
        self.port.lock().unwrap().is_some()
    }

    pub fn get_status(&self) -> ConnectionStatus {
        let port_lock = self.port_name.lock().unwrap();
        ConnectionStatus {
            connected: self.is_connected(),
            port: port_lock.clone(),
            error: None,
        }
    }

    pub async fn start_reading(&self, tx: mpsc::Sender<PotentiometerData>) -> Result<()> {
        let port = self.port.clone();

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 256];
            let mut line_buffer = String::new();

            loop {
                let data_available = {
                    let mut port_guard = port.lock().unwrap();
                    if let Some(ref mut port) = *port_guard {
                        match port.read(&mut buffer) {
                            Ok(n) if n > 0 => {
                                line_buffer.push_str(&String::from_utf8_lossy(&buffer[..n]));
                                true
                            }
                            _ => false,
                        }
                    } else {
                        // Port disconnected
                        break;
                    }
                };

                if data_available {
                    // Process complete lines
                    while let Some(newline_pos) = line_buffer.find('\n') {
                        let line = &line_buffer[..newline_pos];

                        // Try to parse JSON
                        if let Ok(data) = serde_json::from_str::<PotentiometerData>(line) {
                            let _ = tx.send(data).await;
                        }

                        line_buffer.drain(..=newline_pos);
                    }
                }

                sleep(Duration::from_millis(10)).await;
            }
        });

        Ok(())
    }
}

impl Default for SerialManager {
    fn default() -> Self {
        Self::new()
    }
}