// SPDX-License-Identifier: MPL-2.0
//! Explicit virtual controller fixture for native input qualification, not a driver.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use midir::{MidiOutput, MidiOutputConnection, os::unix::VirtualOutput};
    use std::io::{BufRead, Write};
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: midi_controller <unique source name> <new.jsonl>".into());
    }
    let mut log = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[1])?;
    let start = std::time::Instant::now();
    let mut epoch = 0_u64;
    let mut source: Option<MidiOutputConnection> = None;
    for command in std::iter::once(Ok("open".into())).chain(std::io::stdin().lock().lines()) {
        let command = command?;
        let mut parts = command.split_whitespace();
        match parts.next().unwrap_or("") {
            "open" if source.is_none() => {
                epoch += 1;
                source = Some(
                    MidiOutput::new("Pedal input qualification source")?
                        .create_virtual(&args[0])?,
                );
                writeln!(log, "{{\"event\":\"open\",\"epoch\":{epoch}}}")?;
                println!("ready epoch {epoch}");
            },
            "drop" => {
                source.take();
                writeln!(log, "{{\"event\":\"drop\",\"epoch\":{epoch}}}")?;
                println!("dropped epoch {epoch}");
            },
            mode @ ("send" | "sequence") => {
                let bytes: Vec<u8> = parts.map(str::parse).collect::<Result<_, _>>()?;
                if bytes.is_empty()
                    || (mode == "send" && bytes.len() > 3)
                    || (mode == "sequence" && (bytes.len() > 192 || bytes.len() % 3 != 0))
                {
                    return Err("send: 1–3 bytes; sequence: at most 64 three-byte messages".into());
                }
                for message in bytes.chunks(3) {
                    source.as_mut().ok_or("source is closed")?.send(message)?;
                    writeln!(
                        log,
                        "{{\"event\":\"send\",\"epoch\":{epoch},\"micros\":{},\"bytes\":{:?}}}",
                        start.elapsed().as_micros(),
                        message
                    )?;
                }
                println!("sent");
            },
            "quit" => break,
            "" => {},
            _ => return Err("commands: open | drop | send/sequence <decimal bytes> | quit".into()),
        }
        log.flush()?;
        std::io::stdout().flush()?;
    }
    Ok(())
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn main() {
    eprintln!("This virtual controller qualification fixture requires CoreMIDI or ALSA.");
}
