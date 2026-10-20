use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Duration;

use woodshed_audio::MidiOut;

pub(super) enum Command {
    Connect {
        request_id: u64,
        target: Option<String>,
    },
    Enabled(bool),
    Playing(bool),
    Bpm(f32),
    Stop,
}

pub(super) struct Ack {
    pub request_id: u64,
    pub connection_request_id: Option<u64>,
    pub accepted: Option<String>,
    pub error: Option<String>,
}

trait Writer: Send {
    fn start(&mut self) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
    fn tick(&mut self) -> Result<(), String>;
}

trait Connector: Send {
    fn connect(&mut self, name: &str) -> Result<Box<dyn Writer>, String>;
}

struct MidirConnector;

struct MidirWriter(MidiOut);

impl Writer for MidirWriter {
    fn start(&mut self) -> Result<(), String> {
        self.0.send_start().map_err(|e| e.to_string())
    }
    fn stop(&mut self) -> Result<(), String> {
        self.0.send_stop().map_err(|e| e.to_string())
    }
    fn tick(&mut self) -> Result<(), String> {
        self.0.send_clock_tick().map_err(|e| e.to_string())
    }
}

impl Connector for MidirConnector {
    fn connect(&mut self, name: &str) -> Result<Box<dyn Writer>, String> {
        MidiOut::connect_by_name(name)
            .map(|out| Box::new(MidirWriter(out)) as Box<dyn Writer>)
            .map_err(|e| e.to_string())
    }
}

struct Connection {
    name: String,
    writer: Box<dyn Writer>,
    request_id: u64,
}

struct OutputMachine<C: Connector> {
    connector: C,
    connection: Option<Connection>,
    enabled: bool,
    playing: bool,
    bpm: f32,
    transporting: bool,
    request_id: u64,
    reported_write_error: bool,
    write_error: Option<String>,
    reported_error: Option<String>,
}

impl<C: Connector> OutputMachine<C> {
    fn new(connector: C) -> Self {
        Self {
            connector,
            connection: None,
            enabled: false,
            playing: false,
            bpm: 120.0,
            transporting: false,
            request_id: 0,
            reported_write_error: false,
            write_error: None,
            reported_error: None,
        }
    }

    fn wants_transport(&self) -> bool {
        self.enabled && self.playing
    }

    fn set_transport(&mut self) {
        let want = self.wants_transport() && self.connection.is_some();
        if want == self.transporting {
            return;
        }
        if let Some(conn) = self.connection.as_mut() {
            let result = if want {
                conn.writer.start()
            } else {
                conn.writer.stop()
            };
            if let Err(error) = result {
                self.reported_write_error = true;
                self.write_error = Some(error.clone());
                self.reported_error = Some(error);
            }
        }
        self.transporting = want;
    }

    fn connect(&mut self, request_id: u64, target: Option<String>) -> Ack {
        if target.is_none() {
            let mut disconnect_error = None;
            if self.transporting {
                if let Some(old) = self.connection.as_mut() {
                    if let Err(error) = old.writer.stop() {
                        disconnect_error = Some(error);
                    }
                }
            }
            self.connection = None;
            self.transporting = false;
            self.request_id = request_id;
            self.write_error = disconnect_error.clone();
            self.reported_error = disconnect_error.clone();
            if disconnect_error.is_none() {
                self.reported_error = None;
            }
            self.reported_write_error = false;
            return Ack {
                request_id,
                connection_request_id: None,
                accepted: None,
                error: disconnect_error,
            };
        }
        let name = target.expect("checked Some");
        if self
            .connection
            .as_ref()
            .is_some_and(|conn| conn.name == name)
        {
            self.request_id = request_id;
            return Ack {
                request_id,
                connection_request_id: self.connection.as_ref().map(|c| c.request_id),
                accepted: Some(name),
                error: self.reported_error.clone(),
            };
        }
        match self.connector.connect(&name) {
            Ok(writer) => {
                let mut old_stop_error = None;
                if self.transporting {
                    if let Some(old) = self.connection.as_mut() {
                        if let Err(error) = old.writer.stop() {
                            old_stop_error = Some(error);
                        }
                    }
                    self.transporting = false;
                }
                self.connection = Some(Connection {
                    name: name.clone(),
                    writer,
                    request_id,
                });
                self.request_id = request_id;
                self.write_error = None;
                self.reported_error = None;
                self.reported_write_error = false;
                self.set_transport();
                if old_stop_error.is_some() {
                    self.reported_error = old_stop_error.clone();
                    self.write_error = old_stop_error;
                    self.reported_write_error = true;
                }
                Ack {
                    request_id,
                    connection_request_id: Some(request_id),
                    accepted: Some(name),
                    error: self.write_error.take(),
                }
            },
            Err(error) => Ack {
                request_id,
                connection_request_id: self.connection.as_ref().map(|c| c.request_id),
                accepted: self.connection.as_ref().map(|c| c.name.clone()),
                error: Some(error),
            },
        }
    }

    fn apply(&mut self, command: Command) -> Option<Ack> {
        match command {
            Command::Connect { request_id, target } => Some(self.connect(request_id, target)),
            Command::Enabled(enabled) => {
                self.enabled = enabled;
                self.set_transport();
                None
            },
            Command::Playing(playing) => {
                self.playing = playing;
                self.set_transport();
                None
            },
            Command::Bpm(bpm) => {
                self.bpm = bpm;
                None
            },
            Command::Stop => return None,
        }
        None
    }

    fn tick(&mut self) {
        if self.wants_transport() {
            if let Some(conn) = self.connection.as_mut() {
                if !self.reported_write_error {
                    if let Err(error) = conn.writer.tick() {
                        self.reported_write_error = true;
                        self.write_error = Some(error.clone());
                        self.reported_error = Some(error);
                    }
                }
            }
        }
    }

    fn take_write_error(&mut self) -> Option<Ack> {
        self.write_error.take().map(|error| Ack {
            request_id: self.request_id,
            connection_request_id: self.connection.as_ref().map(|conn| conn.request_id),
            accepted: self.connection.as_ref().map(|conn| conn.name.clone()),
            error: Some(error),
        })
    }

    fn interval(&self) -> Duration {
        let bpm = if self.bpm.is_finite() {
            self.bpm.clamp(1.0, 1000.0)
        } else {
            120.0
        };
        Duration::from_secs_f32(60.0 / (bpm * 24.0))
    }

    fn shutdown(&mut self) {
        if self.transporting {
            if let Some(conn) = self.connection.as_mut() {
                let _ = conn.writer.stop();
            }
            self.transporting = false;
        }
    }
}

pub(super) fn clock_out_loop(rx: Receiver<Command>, ack_tx: Sender<Ack>) {
    let mut machine = OutputMachine::new(MidirConnector);
    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::Stop) => {
                    machine.shutdown();
                    return;
                },
                Ok(command) => {
                    if let Some(ack) = machine.apply(command) {
                        let _ = ack_tx.send(ack);
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    machine.shutdown();
                    return;
                },
            }
        }
        machine.tick();
        if let Some(ack) = machine.take_write_error() {
            let _ = ack_tx.send(ack);
        }
        std::thread::sleep(if machine.wants_transport() {
            machine.interval()
        } else {
            Duration::from_millis(20)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct FakeConnector {
        writes: Arc<Mutex<HashMap<String, Vec<&'static str>>>>,
        failures: Vec<String>,
        fail_writes: bool,
    }

    struct FakeWriter {
        name: String,
        writes: Arc<Mutex<HashMap<String, Vec<&'static str>>>>,
        fail_writes: bool,
    }

    impl Writer for FakeWriter {
        fn start(&mut self) -> Result<(), String> {
            self.writes
                .lock()
                .unwrap()
                .entry(self.name.clone())
                .or_default()
                .push("start");
            if self.fail_writes {
                Err("fake writer failed".into())
            } else {
                Ok(())
            }
        }
        fn stop(&mut self) -> Result<(), String> {
            self.writes
                .lock()
                .unwrap()
                .entry(self.name.clone())
                .or_default()
                .push("stop");
            if self.fail_writes {
                Err("fake writer failed".into())
            } else {
                Ok(())
            }
        }
        fn tick(&mut self) -> Result<(), String> {
            self.writes
                .lock()
                .unwrap()
                .entry(self.name.clone())
                .or_default()
                .push("tick");
            if self.fail_writes {
                Err("fake writer failed".into())
            } else {
                Ok(())
            }
        }
    }

    impl Connector for FakeConnector {
        fn connect(&mut self, name: &str) -> Result<Box<dyn Writer>, String> {
            if self.failures.iter().any(|f| f == name) {
                return Err(format!("cannot open {name}"));
            }
            Ok(Box::new(FakeWriter {
                name: name.to_owned(),
                writes: self.writes.clone(),
                fail_writes: self.fail_writes,
            }))
        }
    }

    fn events(connector: &FakeConnector, name: &str) -> Vec<&'static str> {
        connector
            .writes
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn replacing_while_playing_stops_old_and_starts_new() {
        let fake = FakeConnector::default();
        let mut machine = OutputMachine::new(fake.clone());
        machine.apply(Command::Enabled(true));
        machine.apply(Command::Playing(true));
        assert_eq!(
            machine.connect(1, Some("A".into())).accepted.as_deref(),
            Some("A")
        );
        assert_eq!(
            machine.connect(2, Some("B".into())).accepted.as_deref(),
            Some("B")
        );
        assert_eq!(events(&fake, "A"), ["start", "stop"]);
        assert_eq!(events(&fake, "B"), ["start"]);
    }

    #[test]
    fn failed_replacement_retains_and_restarts_old_connection() {
        let mut fake = FakeConnector::default();
        fake.failures.push("missing".into());
        let mut machine = OutputMachine::new(fake.clone());
        machine.apply(Command::Enabled(true));
        machine.apply(Command::Playing(true));
        machine.connect(1, Some("A".into()));
        let ack = machine.connect(2, Some("missing".into()));
        assert_eq!(ack.accepted.as_deref(), Some("A"));
        assert_eq!(ack.error.as_deref(), Some("cannot open missing"));
        assert_eq!(events(&fake, "A"), ["start"]);
        assert_eq!(
            machine.connection.as_ref().map(|c| c.name.as_str()),
            Some("A")
        );
    }

    #[test]
    fn same_accepted_target_is_acknowledged_without_reopening() {
        let fake = FakeConnector::default();
        let mut machine = OutputMachine::new(fake.clone());
        machine.apply(Command::Enabled(true));
        machine.apply(Command::Playing(true));
        machine.connect(1, Some("A".into()));
        let ack = machine.connect(2, Some("A".into()));
        assert_eq!(ack.connection_request_id, Some(1));
        assert_eq!(events(&fake, "A"), ["start"]);
    }

    #[test]
    fn invalid_bpm_keeps_clock_interval_finite() {
        let mut machine = OutputMachine::new(FakeConnector::default());
        machine.apply(Command::Bpm(f32::NAN));
        assert!(machine.interval().as_secs_f32().is_finite());
        machine.apply(Command::Bpm(f32::INFINITY));
        assert!(machine.interval().as_secs_f32().is_finite());
    }

    #[test]
    fn transport_write_error_is_reported_once_per_connection() {
        let fake = FakeConnector {
            fail_writes: true,
            ..FakeConnector::default()
        };
        let mut machine = OutputMachine::new(fake);
        machine.apply(Command::Enabled(true));
        machine.apply(Command::Playing(true));
        let ack = machine.connect(7, Some("A".into()));
        assert_eq!(ack.accepted.as_deref(), Some("A"));
        assert_eq!(ack.error.as_deref(), Some("fake writer failed"));
        machine.tick();
        assert!(machine.take_write_error().is_none());
        let repeated_ack = machine.connect(8, Some("A".into()));
        assert_eq!(repeated_ack.error.as_deref(), Some("fake writer failed"));
    }

    #[test]
    fn disconnect_stops_active_transport_and_clears_connection() {
        let fake = FakeConnector::default();
        let mut machine = OutputMachine::new(fake.clone());
        machine.apply(Command::Enabled(true));
        machine.apply(Command::Playing(true));
        machine.connect(1, Some("A".into()));
        assert_eq!(machine.connect(2, None).accepted, None);
        assert_eq!(events(&fake, "A"), ["start", "stop"]);
        assert!(machine.connection.is_none());
    }
}
