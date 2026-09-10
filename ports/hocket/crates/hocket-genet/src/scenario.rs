// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The self-drive scenario lane, adopted from turnstone.
//!
//! Hocket drives ITSELF from a scenario file, so a headed receipt needs no OS
//! synthetic input: no focus race, no key theft while a human uses the machine.
//! Set `HOCKET_SCENARIO` to a scenario path before launch; captures and the
//! receipt land in `HOCKET_CAPTURE_DIR` (default: beside the scenario). The run
//! writes `scenario.done` (first line `RESULT ok|fail`, then the log) and exits
//! without saving, so a scenario never mutates a profile and reruns stay
//! deterministic.
//!
//! The grammar is genet-probe's shared one, driven through the [`Automatable`]
//! and [`Driveable`] impls on the host: `settle`, `click role:name text` /
//! `click .class text`, `capture <name>`, `assert text <substr>`,
//! `assert snap <field> <op> <value>`, `log`. Hocket adds no app-specific verbs
//! yet, so an unknown verb fails loudly through the default `app_step`.

use std::path::{Path, PathBuf};

use cambium_genet_winit_host::Surface;
use image::ImageEncoder;
use netrender::ExternalTexturePlacement;

pub use genet_probe::{Outcome, Scenario};

/// A loaded scenario plus the directory its captures and receipt land in.
pub struct Run {
    pub scenario: Scenario,
    pub dir: PathBuf,
}

/// Load the scenario named by `HOCKET_SCENARIO`, or `None` for a normal launch.
/// A malformed scenario panics at startup rather than launching into a run that
/// cannot mean anything.
pub fn load() -> Option<Run> {
    let path = PathBuf::from(std::env::var_os("HOCKET_SCENARIO")?);
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read scenario {path:?}: {error}"));
    let scenario =
        Scenario::parse(&body).unwrap_or_else(|error| panic!("parse scenario {path:?}: {error}"));
    let dir = std::env::var_os("HOCKET_CAPTURE_DIR")
        .map(PathBuf::from)
        .or_else(|| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    Some(Run { scenario, dir })
}

/// Write the run receipt: `RESULT ok|fail`, then the log lines.
pub fn write_done(dir: &Path, outcome: &Outcome) {
    let mut text = format!("RESULT {}\n", if outcome.ok { "ok" } else { "fail" });
    for line in &outcome.log {
        text.push_str(line);
        text.push('\n');
    }
    let _ = std::fs::write(dir.join("scenario.done"), text);
}

/// Self-capture the composed frame to a PNG. Composes the same rasterized scene
/// `view` a normal frame presents into a `COPY_SRC` target and reads it back, so
/// the receipt is the presented frame, immune to occlusion and focus theft.
pub fn capture_frame(
    surface: &dyn Surface,
    view: &wgpu::TextureView,
    width: u32,
    height: u32,
    path: &Path,
) -> bool {
    let target = surface.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("hocket scenario capture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
    surface.renderer().compose_external_texture(
        view,
        &target_view,
        wgpu::TextureFormat::Rgba8Unorm,
        width,
        height,
        ExternalTexturePlacement::new([0.0, 0.0, width as f32, height as f32]),
    );
    let rgba = read_texture_rgba(surface.device(), surface.queue(), &target, width, height);
    if rgba.is_empty() {
        return false;
    }
    let Ok(file) = std::fs::File::create(path) else {
        return false;
    };
    image::codecs::png::PngEncoder::new(file)
        .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
        .is_ok()
}

/// Read a texture back as tightly packed RGBA8 (empty on failure). Standard wgpu
/// readback: copy into a row-aligned buffer, map, strip the per-row padding.
fn read_texture_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let row_bytes = width * 4;
    let padded = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hocket capture readback"),
        size: padded as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("hocket capture readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    if device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
        return Vec::new();
    }
    if !matches!(rx.recv(), Ok(Ok(()))) {
        return Vec::new();
    }
    let Ok(mapped) = slice.get_mapped_range() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity((row_bytes * height) as usize);
    for row in 0..height as usize {
        let start = row * padded as usize;
        out.extend_from_slice(&mapped[start..start + row_bytes as usize]);
    }
    out
}
