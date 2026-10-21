//! Opt-in JSONL control of the running desktop owner. No second MIDI session.
use super::{PedalAction, PedalOutcome, PedalReport, pedal::Dispatcher};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::time::Duration;

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
enum Command {
    Status {
        id: String,
    },
    Trigger {
        id: String,
        action: PedalAction,
        output_revision: u64,
        /// Required JSON key; null for an OS port, integer for BLE.
        generation: serde_json::Value,
    },
}
pub(super) fn start(dispatcher: Dispatcher) {
    std::thread::spawn(move || {
        run(
            &dispatcher,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout(),
        )
    });
}

/// The production stream loop is also exercised with bounded in-memory streams.
fn run(dispatcher: &Dispatcher, input: &mut impl BufRead, output: &mut impl Write) {
    let mut used = HashSet::new();
    loop {
        let line = match read_line(input) {
            Ok(Some(line)) => line,
            Ok(None) => return,
            Err(error) => {
                let terminal = error.kind() != std::io::ErrorKind::InvalidData;
                if writeln!(output, "{}", json!({"id":null,"error":error.to_string()}))
                    .and_then(|_| output.flush())
                    .is_err()
                {
                    return;
                }
                if terminal {
                    return;
                }
                continue;
            },
        };
        let response = match serde_json::from_slice::<Command>(&line) {
            Err(error) => json!({"id":null,"error":error.to_string()}),
            Ok(command) => {
                let id = match &command {
                    Command::Status { id } | Command::Trigger { id, .. } => id.clone(),
                };
                if id.is_empty() || id.len() > 128 {
                    json!({"id":id,"error":"id must be 1–128 UTF-8 bytes"})
                } else if used.contains(&id) {
                    json!({"id":id,"error":"Duplicate id refused; no action replayed"})
                } else if used.len() >= 4096 {
                    json!({"id":id,"error":"4096-command session limit reached; no action sent"})
                } else {
                    used.insert(id.clone());
                    match command {
                        Command::Status { .. } => {
                            let mut status = dispatcher.status();
                            status["id"] = json!(id);
                            status
                        },
                        Command::Trigger {
                            action,
                            output_revision,
                            generation,
                            ..
                        } => {
                            let generation = if generation.is_null() {
                                None
                            } else if let Some(generation) = generation.as_u64() {
                                Some(generation)
                            } else {
                                let response = json!({"id":id,"error":"generation must be null or an unsigned integer"});
                                if writeln!(output, "{response}")
                                    .and_then(|_| output.flush())
                                    .is_err()
                                {
                                    return;
                                }
                                continue;
                            };
                            let binding = dispatcher.bindings.lock().unwrap().get(action).clone();
                            let (queued, reply) = dispatcher.submit_expected(
                                action,
                                binding,
                                Some((output_revision, generation)),
                            );
                            let report = match reply {
                                None => queued,
                                Some(reply) => reply
                                    .recv_timeout(Duration::from_secs(3))
                                    .unwrap_or_else(|_| PedalReport {
                                        outcome: PedalOutcome::Uncertain,
                                        detail:
                                            "Completion not received; do not retry automatically"
                                                .into(),
                                        ..queued
                                    }),
                            };
                            json!({"id":id,"pedal":report})
                        },
                    }
                }
            },
        };
        if writeln!(output, "{response}")
            .and_then(|_| output.flush())
            .is_err()
        {
            return;
        }
    }
}

/// Bound each line before allocation; drain an oversized line so it cannot
/// become several commands. EOF closes the input lane, not the desktop app.
fn read_line(input: &mut impl BufRead) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let mut oversized = false;
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            return if oversized {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "JSON line exceeds 4096 bytes",
                ))
            } else if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        let end = buf.iter().position(|b| *b == b'\n');
        let count = end.map_or(buf.len(), |i| i + 1);
        if line.len() + count > 4096 {
            oversized = true;
        }
        if !oversized {
            line.extend_from_slice(&buf[..count]);
        }
        input.consume(count);
        if end.is_some() {
            return if oversized {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "JSON line exceeds 4096 bytes",
                ))
            } else {
                Ok(Some(line))
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stream(
        input: &str,
    ) -> (
        Vec<serde_json::Value>,
        std::sync::mpsc::Receiver<super::super::ClockCmd>,
    ) {
        let (d, rx) = super::super::pedal::fixture();
        let mut output = Vec::new();
        run(&d, &mut std::io::Cursor::new(input), &mut output);
        (
            String::from_utf8(output)
                .unwrap()
                .lines()
                .map(|s| serde_json::from_str(s).unwrap())
                .collect(),
            rx,
        )
    }
    #[test]
    fn pedal_stdio_terminal_input_error_stops_after_one_report() {
        struct Broken;
        impl std::io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("broken input"))
            }
        }
        impl std::io::BufRead for Broken {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                Err(std::io::Error::other("broken input"))
            }
            fn consume(&mut self, _: usize) {}
        }
        let (d, _) = super::super::pedal::fixture();
        let mut output = Vec::new();
        run(&d, &mut Broken, &mut output);
        let lines = String::from_utf8(output).unwrap();
        assert_eq!(lines.lines().count(), 1);
        assert!(lines.contains("broken input"));
    }
    #[test]
    fn pedal_stdio_status_duplicate_id_and_disabled_trigger_never_send() {
        let (result, rx) = stream(concat!(
            "{\"id\":\"s\",\"command\":\"status\"}\n",
            "{\"id\":\"s\",\"command\":\"status\"}\n",
            "{\"id\":\"t\",\"command\":\"trigger\",\"action\":\"effect-next\",\"output_revision\":1,\"generation\":7}\n"
        ));
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["output"]["generation"], 7);
        assert!(result[1]["error"].as_str().unwrap().contains("Duplicate"));
        assert_eq!(result[2]["pedal"]["outcome"], "refused");
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn pedal_stdio_rejects_missing_target_unknown_action_and_extra_fields() {
        for input in [
            r#"{"id":"x","command":"trigger","action":"effect-next","output_revision":1}"#,
            r#"{"id":"x","command":"trigger","action":"loop-erase","output_revision":1,"generation":7}"#,
            r#"{"id":"x","command":"status","extra":true}"#,
        ] {
            let (result, rx) = stream(input);
            assert!(!result[0]["error"].is_null(), "{input}: {result:?}");
            assert!(rx.try_recv().is_err());
        }
    }
    #[test]
    fn pedal_stdio_oversized_line_is_drained_and_next_command_survives() {
        let input = "x".repeat(4097) + "\n" + r#"{"id":"s","command":"status"}"#;
        let (result, rx) = stream(&input);
        assert_eq!(result.len(), 2);
        assert!(result[0]["error"].as_str().unwrap().contains("4096"));
        assert_eq!(result[1]["id"], "s");
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn pedal_stdio_invalid_json_and_generation_and_id_lengths_refuse() {
        for input in ["oops".to_owned(), r#"{"id":"","command":"status"}"#.to_owned(),
            serde_json::json!({"id":"x".repeat(129),"command":"status"}).to_string(),
            r#"{"id":"x","command":"trigger","action":"effect-next","output_revision":1,"generation":-1}"#.to_owned()] {
            let (result,rx)=stream(&input);assert!(!result[0]["error"].is_null());assert!(rx.try_recv().is_err());
        }
    }
}
