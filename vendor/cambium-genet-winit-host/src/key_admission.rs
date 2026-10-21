// SPDX-License-Identifier: MPL-2.0

/// Winit synthesizes held-key snapshots on native focus transitions. Those
/// presses must not enter editing, button activation or application shortcuts.
pub(crate) fn fresh_key_press(is_synthetic: bool) -> bool {
    !is_synthetic
}

#[cfg(test)]
mod focus_snapshot_tests {
    #[test]
    fn synthetic_held_key_snapshots_never_enter_application_dispatch() {
        assert!(super::fresh_key_press(false));
        for _ in 0..64 {
            assert!(!super::fresh_key_press(true));
        }
    }
}
