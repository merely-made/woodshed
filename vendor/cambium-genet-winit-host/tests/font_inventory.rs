// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
use cambium_genet_winit_host::{HostFont, available_font_families};
#[test]
fn family_inventory_uses_registered_metadata_and_rejects_invalid_bytes() {
    let family = "TabardInventorySyntheticFace";
    let font = HostFont {
        family: Some(family.into()),
        bytes: include_bytes!("../../examples/genet_web_smoke/assets/Roboto-Regular.ttf").to_vec(),
    };
    assert!(
        !available_font_families(&[])
            .iter()
            .any(|name| name == family)
    );
    assert!(
        available_font_families(&[font])
            .iter()
            .any(|name| name == family)
    );
    let invalid = HostFont {
        family: Some("TabardInventoryInvalidFace".into()),
        bytes: vec![0, 1, 2],
    };
    assert!(
        !available_font_families(&[invalid])
            .iter()
            .any(|name| name == "TabardInventoryInvalidFace")
    );
}
