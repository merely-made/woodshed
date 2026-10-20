//! Minimal file-reading host for woodshedding's portable musical timing.
use std::{env, fmt::Write, fs, process::ExitCode};
use woodshedding::{
    notation::MusicDocument,
    timing::{ArrangementSchedule, TICKS_PER_QUARTER},
};

fn run() -> Result<String, String> {
    let mut args = env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("Usage: musicxml-timing FILE.musicxml [BPM]")?;
    let tempo = args.next();
    if args.next().is_some() {
        return Err("Usage: musicxml-timing FILE.musicxml [BPM]".into());
    }
    let document = MusicDocument {
        musicxml: fs::read_to_string(path).map_err(|e| format!("Read MusicXML: {e}"))?,
    };
    let score = document.score()?;
    let tempo = match tempo {
        Some(value) => value
            .to_str()
            .ok_or("BPM must be UTF-8")?
            .parse::<f64>()
            .map_err(|_| "BPM must be a number")?,
        None => f64::from(score.tempo),
    };
    let timing = ArrangementSchedule::from_score(&score, tempo)?;
    let mut output = format!(
        "{}\n{} | {} strings | written {} BPM | timing {} BPM | {} ticks/quarter\n",
        score.title,
        score.instrument,
        score.tuning.len(),
        score.tempo,
        timing.tempo_bpm,
        TICKS_PER_QUARTER,
    );
    for (measure, timed) in score.measures.iter().zip(&timing.measures) {
        writeln!(
            output,
            "bar {} | {}/{} | {} ticks",
            timed.measure_index + 1,
            measure.meter.beats,
            measure.meter.beat_type,
            timed.duration_ticks
        )
        .unwrap();
        for event in &timed.events {
            write!(
                output,
                "  event {} | onset {} | duration {} |",
                event.event_index + 1,
                event.onset_ticks,
                event.gate_ticks
            )
            .unwrap();
            if event.is_rest() {
                output.push_str(" rest");
            }
            for note in &event.notes {
                write!(
                    output,
                    " MIDI {} string {} fret {} ({:.3} Hz)",
                    note.midi, note.contact.string, note.contact.fret, note.frequency_hz
                )
                .unwrap();
            }
            output.push('\n');
        }
    }
    Ok(output)
}

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        },
        Err(error) => {
            eprintln!("musicxml-timing: {error}");
            ExitCode::FAILURE
        },
    }
}
