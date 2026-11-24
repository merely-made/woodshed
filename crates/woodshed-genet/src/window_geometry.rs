//! Window placement: the host's geometry and the saved desktop preference.

use std::cell::RefCell;
use std::rc::Rc;

use woodshed_core::settings::WindowSettings;

use crate::desktop;
use crate::shared::Shared;
use crate::sync::Ctx;

pub(crate) fn to_host_geometry(settings: WindowSettings) -> cambium_genet_winit_host::WindowGeometry {
    cambium_genet_winit_host::WindowGeometry {
        position: (settings.x, settings.y),
        size: (settings.width, settings.height),
        maximized: settings.maximized,
    }
}

pub(crate) fn to_window_settings(geometry: cambium_genet_winit_host::WindowGeometry) -> WindowSettings {
    WindowSettings {
        x: geometry.position.0,
        y: geometry.position.1,
        width: geometry.size.0,
        height: geometry.size.1,
        maximized: geometry.maximized,
    }
}

pub(crate) fn initial_window_geometry(
    shared: &Rc<RefCell<Shared>>,
) -> Option<cambium_genet_winit_host::WindowGeometry> {
    let shared = shared.borrow();
    let root = shared.data_root.as_deref()?;
    match desktop::load(root) {
        Ok(settings) => settings.window.map(to_host_geometry),
        Err(error) => {
            eprintln!("[woodshed] desktop preferences could not open: {error}");
            None
        },
    }
}

pub(crate) fn persist_window_geometry(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    geometry: cambium_genet_winit_host::WindowGeometry,
) {
    let Some(root) = shared.data_root.as_deref() else {
        return;
    };
    let settings = desktop::DesktopSettings {
        window: Some(to_window_settings(geometry)),
    };
    if let Err(error) = desktop::save(root, &settings) {
        eprintln!("[woodshed] window placement was not saved: {error}");
        ctx.runner.update(|ui| {
            ui.appearance_notice = Some(format!("Window placement was not saved: {error}"))
        });
    }
}
