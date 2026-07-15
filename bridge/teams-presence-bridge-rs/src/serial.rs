use std::io::{Read, Write};
use std::time::Duration;
use serialport::{SerialPort, SerialPortType};

pub struct SerialManager {
    port: Option<Box<dyn SerialPort>>,
    active_port_name: Option<String>,
}

impl SerialManager {
    pub fn new() -> Self {
        Self { port: None, active_port_name: None }
    }

    pub fn get_port_name(&self) -> Option<String> {
        self.active_port_name.clone()
    }

    pub fn connect(&mut self, port_name: &str) -> bool {
        let name = if port_name == "AUTO" {
            Self::auto_detect_port()
        } else {
            Some(port_name.to_string())
        };

        if let Some(p) = name {
            match serialport::new(&p, 115200)
                .timeout(Duration::from_millis(100))
                .open()
            {
                Ok(port) => {
                    println!("[SerialManager] Connected to {}", p);
                    self.port = Some(port);
                    self.active_port_name = Some(p);
                    true
                }
                Err(e) => {
                    eprintln!("[SerialManager] Failed to open port {}: {}", p, e);
                    self.active_port_name = None;
                    false
                }
            }
        } else {
            eprintln!("[SerialManager] No COM port found");
            false
        }
    }

    pub fn is_connected(&self) -> bool {
        self.port.is_some()
    }

    pub fn send_command(&mut self, cmd: &str) {
        if let Some(ref mut port) = self.port {
            if let Err(e) = port.write_all(cmd.as_bytes()) {
                eprintln!("[SerialManager] Error writing to serial: {}", e);
                self.port = None; // Disconnect on error
            }
        }
    }

    pub fn send_ping(&mut self) {
        // Drain any stale OK/PONG responses sitting in the buffer
        if let Some(ref mut port) = self.port {
            let mut drain = [0u8; 256];
            while port.read(&mut drain).unwrap_or(0) > 0 {}
        }
        self.send_command("PING\n");
        if let Some(ref mut port) = self.port {
            let mut buf = [0u8; 64];
            if let Err(e) = port.read(&mut buf) {
                eprintln!("[SerialManager] Error reading PONG, disconnecting: {}", e);
                self.port = None;
            }
        }
    }

    pub fn send_brightness(&mut self, value: u8) {
        let cmd = format!("BRIGHTNESS:{}\n", value);
        self.send_command(&cmd);
    }

    pub fn send_transition(&mut self, value: u16) {
        let cmd = format!("TRANSITION:{}\n", value);
        self.send_command(&cmd);
    }

    fn auto_detect_port() -> Option<String> {
        let ports = serialport::available_ports().unwrap_or_default();
        
        // Find the first USB port if possible
        for p in &ports {
            if let SerialPortType::UsbPort(_info) = &p.port_type {
                // We could filter by VID/PID of standard ESP32 boards here
                // For now, just return the first USB port
                return Some(p.port_name.clone());
            }
        }
        
        // Fallback to the last available port (similar to C# logic)
        ports.last().map(|p| p.port_name.clone())
    }
}
