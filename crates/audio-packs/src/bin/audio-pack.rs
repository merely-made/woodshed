// SPDX-License-Identifier: MPL-2.0
fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match audio_packs::files::run_cli(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => { eprintln!("{error}"); std::process::ExitCode::FAILURE }
    }
}
