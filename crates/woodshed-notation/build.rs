// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! With feature `block-guest`, materialises the `mere:script` WIT world as `wit/world.wit`
//! (ignored by git) for `wit_bindgen::generate!`, which takes only literal paths.
//!
//! The world lives once, in Graphshell's `graphshell-endpoint`; this reads the directory
//! that package publishes as Cargo `links` metadata (`DEP_GRAPHSHELL_ENDPOINT_WIT`, passed
//! only from normal dependencies). It is a build product of the dependency, not a second
//! copy to keep in step.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_BLOCK_GUEST").is_none() {
        return;
    }
    let dir = std::env::var("DEP_GRAPHSHELL_ENDPOINT_WIT")
        .expect("graphshell-endpoint's build metadata names its wit directory");
    let source = std::path::Path::new(&dir).join("world.wit");
    println!("cargo::rerun-if-changed={}", source.display());
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = std::path::Path::new(&root).join("wit");
    let target = target_dir.join("world.wit");
    let wanted = std::fs::read(&source).expect("read graphshell-endpoint's WIT world");
    if std::fs::read(&target).ok().as_deref() == Some(wanted.as_slice()) {
        return;
    }
    std::fs::create_dir_all(&target_dir).expect("create wit directory");
    std::fs::write(&target, wanted).expect("write wit/world.wit");
}
