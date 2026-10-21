// SPDX-License-Identifier: MPL-2.0
//! Receiving virtual MIDI fixture for native desktop qualification. Never sends.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use midir::{Ignore, MidiInput, MidiInputConnection, os::unix::VirtualInput};
    use std::{
        io::{BufRead, Write},
        sync::{Arc, Mutex},
        time::Instant,
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: midi_capture <unique port name> <new.jsonl>".into());
    }
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[1])?;
    let log = Arc::new(Mutex::new(file));
    let start = Instant::now();
    let mut epoch = 0_u64;
    let mut receiver: Option<MidiInputConnection<()>> = None;
    for command in std::iter::once(Ok("open".into())).chain(std::io::stdin().lock().lines()) {
        match command?.trim() {
            "open" if receiver.is_none() => {
                epoch += 1;
                let mut input = MidiInput::new("Desktop MIDI receiving fixture")?;
                input.ignore(Ignore::None);
                let output = log.clone();
                let generation = epoch;
                receiver = Some(input.create_virtual(
                    &args[0],
                    move |_, bytes, _| {
                        let mut file = output.lock().unwrap();
                        writeln!(
                            file,
                            "{{\"event\":\"packet\",\"epoch\":{},\"micros\":{},\"bytes\":{:?}}}",
                            generation,
                            start.elapsed().as_micros(),
                            bytes
                        )
                        .unwrap();
                        file.flush().unwrap();
                    },
                    (),
                )?);
                writeln!(
                    log.lock().unwrap(),
                    "{{\"event\":\"open\",\"epoch\":{epoch}}}"
                )?;
                println!("ready epoch {epoch}");
            },
            "drop" => {
                receiver.take();
                writeln!(
                    log.lock().unwrap(),
                    "{{\"event\":\"drop\",\"epoch\":{epoch}}}"
                )?;
                println!("dropped epoch {epoch}");
            },
            "quit" => break,
            "" => {},
            _ => eprintln!("commands: open | drop | quit"),
        }
        log.lock().unwrap().flush()?;
    }
    Ok(())
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn main() {
    eprintln!("This receiving virtual-port fixture requires CoreMIDI or ALSA.");
}
