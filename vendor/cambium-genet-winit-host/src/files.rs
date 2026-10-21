/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The desktop's file choosers: platform open and save-path dialogs, through
//! `light-file-dialog`, the dialog crate Graphshell's desktop already uses.
//!
//! The dialog is modal and answers before [`FileChooser::open`] returns. A
//! machine with no graphical dialog backend answers with nothing chosen.

use std::path::PathBuf;

use cambium::{FileEvent, FileRequest, OpenedFile};
use cambium_rootstock::{FileAnswer, FileChooser};

/// Opens files through the platform's modal open dialog.
#[derive(Default)]
pub struct DialogFileChooser;

impl FileChooser for DialogFileChooser {
    fn open(&mut self, request: &FileRequest, answer: FileAnswer) {
        answer.send(FileEvent {
            files: choose(request),
        });
    }
}

fn choose(request: &FileRequest) -> Vec<OpenedFile> {
    use light_file_dialog::dialog::{Dialog, OpenFileDialog};

    if !graphical_dialog_available() {
        return Vec::new();
    }
    let patterns: Vec<String> = request
        .filter
        .extensions
        .iter()
        .map(|extension| format!("*.{extension}"))
        .collect();
    let patterns: Vec<&str> = patterns.iter().map(String::as_str).collect();
    let mut dialog = OpenFileDialog::new("Open").filter(&patterns);
    if request.filter.multiple {
        dialog = dialog.multiple();
    }
    // Several chosen paths come back joined by `|`.
    dialog
        .show()
        .map(|chosen| {
            chosen
                .split('|')
                .filter(|path| !path.is_empty())
                .filter_map(|path| read_file(std::path::Path::new(path)))
                .collect()
        })
        .unwrap_or_default()
}

/// Choose an output path through the platform's modal save dialog.
///
/// `extensions` contains suffixes without a leading dot, as in `&["json"]`.
/// Cancellation or an unavailable graphical backend returns `None`. This
/// chooses only a path: validation, replacement policy and writing remain the
/// caller's responsibility.
pub fn choose_save_path(title: &str, suggested_name: &str, extensions: &[&str]) -> Option<PathBuf> {
    use light_file_dialog::dialog::{Dialog, SaveFileDialog};

    if !graphical_dialog_available() {
        return None;
    }
    let patterns: Vec<String> = extensions
        .iter()
        .map(|extension| format!("*.{extension}"))
        .collect();
    let patterns: Vec<&str> = patterns.iter().map(String::as_str).collect();
    SaveFileDialog::new(title)
        .default_path(suggested_name)
        .filter(&patterns)
        .show()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

fn graphical_dialog_available() -> bool {
    use light_file_dialog::dialog::DialogBackend;

    light_file_dialog::set_verbose(0);
    light_file_dialog::set_silent(1);
    light_file_dialog::set_force_console(0);
    DialogBackend::query().graphic
}

/// A file on disk as an [`OpenedFile`], or `None` when it cannot be read.
pub fn read_file(path: &std::path::Path) -> Option<OpenedFile> {
    let bytes = std::fs::read(path).ok()?;
    let last_modified_ms = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_millis() as u64);
    Some(OpenedFile {
        name: path.file_name()?.to_string_lossy().into_owned(),
        media_type: None,
        last_modified_ms,
        bytes,
    })
}
