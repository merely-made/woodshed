use super::*;

impl MidiOut {
    pub fn connect_by_name(port_name: &str) -> Result<Self, MidiError> {
        #[cfg(feature = "ble-midi")]
        if port_name == ble::OUTPUT_NAME || port_name == ble::COMPAT_OUTPUT_NAME {
            let mut config = ble::Config::default();
            if port_name == ble::COMPAT_OUTPUT_NAME {
                config.security = ble::Security::UnencryptedCompatibility;
            }
            return Self::advertise_ble(config);
        }
        Self::connect_port_by_name(port_name)
    }

    /// Open an OS port explicitly, including one whose name matches a transport label.
    pub fn connect_port_by_name(port_name: &str) -> Result<Self, MidiError> {
        let output = MidiOutput::new("woodshed-out")?;
        let port = find_output_port(&output, port_name)?;
        let inventory = MidiOutput::new("woodshed-output-inventory")?;
        let id = port.id();
        let conn = output
            .connect(&port, "woodshed-out")
            .map_err(|e| MidiError::Connect(format!("{e}")))?;
        Ok(Self {
            conn: OutputConnection::Port(PortOutput {
                connection: conn,
                port,
                id,
                inventory,
            }),
        })
    }

    /// Send raw bytes. Prefer the named helpers below where they
    /// exist; this is the escape hatch for messages we don't model.
    pub fn send_raw(&mut self, bytes: &[u8]) -> Result<(), MidiError> {
        match &mut self.conn {
            OutputConnection::Port(connection) => {
                connection.send(bytes)?;
                Ok(())
            },
            #[cfg(all(
                feature = "ble-midi",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            ))]
            OutputConnection::Bluetooth(connection) => {
                connection.send(bytes).map_err(MidiError::Bluetooth)
            },
        }
    }

    /// Send only to this output instance's expected subscriber generation.
    /// A native OS port has no subscription generation and uses `None`.
    pub fn send_raw_for_generation(
        &mut self,
        bytes: &[u8],
        generation: Option<u64>,
    ) -> Result<(), MidiError> {
        match &mut self.conn {
            OutputConnection::Port(connection) => {
                connection.send(bytes)?;
                Ok(())
            },
            #[cfg(all(
                feature = "ble-midi",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            ))]
            OutputConnection::Bluetooth(connection) => connection
                .send_for_generation(bytes, generation)
                .map_err(MidiError::Bluetooth),
        }
    }
    pub fn connection_generation(&self) -> Option<u64> {
        #[cfg(feature = "ble-midi")]
        {
            self.ble_status().map(|status| status.generation)
        }
        #[cfg(not(feature = "ble-midi"))]
        {
            None
        }
    }

    /// Advertise the standard BLE-MIDI service. Construction sends no MIDI.
    /// The caller chooses security explicitly; the default requires encryption.
    #[cfg(feature = "ble-midi")]
    pub fn advertise_ble(config: ble::Config) -> Result<Self, MidiError> {
        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        {
            ble::Output::open(config)
                .map(|connection| Self {
                    conn: OutputConnection::Bluetooth(connection),
                })
                .map_err(MidiError::Bluetooth)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            let _ = config;
            Err(MidiError::Bluetooth(ble::Error::Unsupported))
        }
    }

    #[cfg(feature = "ble-midi")]
    pub fn ble_status(&self) -> Option<ble::Status> {
        match &self.conn {
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            OutputConnection::Bluetooth(connection) => Some(connection.status()),
            _ => None,
        }
    }

    /// Transport readiness for fresh output; native MIDI ports have no peer subscription.
    pub fn is_ready(&self) -> bool {
        match &self.conn {
            OutputConnection::Port(connection) => connection.available(),
            #[cfg(all(
                feature = "ble-midi",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            ))]
            OutputConnection::Bluetooth(connection) => {
                let status = connection.status();
                status.phase == ble::Phase::Subscribed && status.subscribers == 1
            },
        }
    }

    /// Human-readable transport state for the existing host's output readout.
    pub fn transport_status(&self) -> Option<String> {
        match &self.conn {
            OutputConnection::Port(connection) => {
                if connection.available() {
                    None
                } else {
                    Some("MIDI destination is unavailable; reconnect explicitly".into())
                }
            },
            #[cfg(all(
                feature = "ble-midi",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            ))]
            OutputConnection::Bluetooth(connection) => {
                let status = connection.status();
                Some(status.last_error.unwrap_or_else(|| match status.phase {
                    ble::Phase::Advertising => "Advertising; waiting for a MIDI receiver".into(),
                    ble::Phase::Subscribed => match status.security {
                        ble::Security::Encrypted => {
                            "MIDI receiver subscribed (encryption required)".into()
                        },
                        ble::Security::UnencryptedCompatibility => {
                            "MIDI receiver subscribed (unencrypted compatibility)".into()
                        },
                    },
                    ble::Phase::Unavailable => "Bluetooth MIDI is unavailable".into(),
                    ble::Phase::Closed => "Bluetooth MIDI stopped".into(),
                }))
            },
        }
    }

    /// Explicitly close the selected output, reporting BLE cleanup failures.
    pub fn close(mut self) -> Result<(), MidiError> {
        match &mut self.conn {
            OutputConnection::Port(_) => Ok(()),
            #[cfg(all(
                feature = "ble-midi",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            ))]
            OutputConnection::Bluetooth(connection) => {
                connection.close().map_err(MidiError::Bluetooth)
            },
        }
    }

    pub fn send_note_on(&mut self, channel: u8, note: u8, velocity: u8) -> Result<(), MidiError> {
        self.send_raw(&[0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F])
    }

    pub fn send_note_off(&mut self, channel: u8, note: u8, velocity: u8) -> Result<(), MidiError> {
        self.send_raw(&[0x80 | (channel & 0x0F), note & 0x7F, velocity & 0x7F])
    }

    pub fn send_clock_tick(&mut self) -> Result<(), MidiError> {
        self.send_raw(&[CLOCK_TICK])
    }

    pub fn send_start(&mut self) -> Result<(), MidiError> {
        self.send_raw(&[TRANSPORT_START])
    }

    pub fn send_stop(&mut self) -> Result<(), MidiError> {
        self.send_raw(&[TRANSPORT_STOP])
    }

    pub fn send_continue(&mut self) -> Result<(), MidiError> {
        self.send_raw(&[TRANSPORT_CONTINUE])
    }
}
