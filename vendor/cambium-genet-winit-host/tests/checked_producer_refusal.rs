// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Actual SceneProducer/registry refusal gates. Headless GPU resources qualify
//! invalidation and typed host refusal, not native surface presentation.
#![cfg(not(target_arch = "wasm32"))]

use cambium::{AnyView, GenetCtx, GenetElement, custom_leaf};
use cambium_genet_winit_host::SceneProducer;
use cambium_rootstock::{
    CaptureRefusal, Host, HostHooks, HostOptions, HostState, HostWake, HostWindow,
    PortableProducerError, ProducedScene, ProducedTexture, ProducerContext, ProducerError,
    ProducerFrameInfo, Runner, ScriptedDom, Surface, TextureProducer,
};
use genet_render_host::{RasterizeFailure, RecoveryStatus, RenderCore, SharedRenderCore};
use netrender::{NetrenderOptions, Scene, VelloError, external_image_key};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc, Weak,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const KEY: u64 = 807;
const WIDTH: u32 = 32;
const HEIGHT: u32 = 24;
struct Source {
    revision: u64,
    destroy: bool,
    refuse: bool,
    frames: usize,
    core: Weak<RenderCore>,
}
fn frame(source: &mut Source, width: u32, height: u32) -> Scene {
    source.frames += 1;
    if source.destroy {
        source.destroy = false;
        let core = source.core.upgrade().expect("host owns the actual device");
        core.device().destroy();
        let deadline = Instant::now() + Duration::from_secs(5);
        while core.device_failure().is_none() {
            assert!(
                Instant::now() < deadline,
                "actual loss callback exceeded five seconds"
            );
            let _ = core.device().poll(wgpu::PollType::Poll);
            std::thread::yield_now();
        }
    }
    let mut scene = Scene::new(width, height);
    scene.push_rect(
        0.0,
        0.0,
        width as f32,
        height as f32,
        [0.25, 0.5, 0.75, 1.0],
    );
    scene
}
fn checked_frame(source: &mut Source, width: u32, height: u32) -> Result<Scene, String> {
    if source.refuse {
        return Err("authored scene refused".into());
    }
    Ok(frame(source, width, height))
}
fn revision(source: &Source) -> u64 {
    source.revision
}
struct State {
    draft: String,
    source: Rc<RefCell<Source>>,
}
type Child = Box<dyn AnyView<State, (), GenetCtx, GenetElement>>;
type Logic = fn(&State) -> Child;
type TestHost = Host<State, Logic, Child>;
fn root(_: &State) -> Child {
    Box::new(custom_leaf::<State, ()>(KEY, WIDTH, HEIGHT))
}
struct Window;
impl HostWindow for Window {
    fn request_redraw(&self) {}
    fn inner_size(&self) -> (u32, u32) {
        (WIDTH, HEIGHT)
    }
    fn scale_factor(&self) -> f64 {
        1.0
    }
    fn set_ime_allowed(&self, _: bool) {}
    fn set_ime_cursor_area(&self, _: f64, _: f64, _: f64, _: f64) {}
}
struct HeadlessSurface(Arc<RenderCore>);
impl Surface for HeadlessSurface {
    fn core(&self) -> &RenderCore {
        &self.0
    }
    fn format(&self) -> wgpu::TextureFormat {
        wgpu::TextureFormat::Rgba8Unorm
    }
    fn resize(&mut self, _: u32, _: u32) {}
    fn acquire(&self) -> Option<wgpu::SurfaceTexture> {
        None
    }
}
struct RejectNext {
    producer: SceneProducer<Source>,
    reject: Rc<Cell<bool>>,
}
impl TextureProducer for RejectNext {
    fn render(&mut self, cx: &ProducerContext<'_>) -> Option<ProducedTexture> {
        self.producer.render(cx)
    }
    fn render_checked(
        &mut self,
        cx: &ProducerContext<'_>,
    ) -> Result<Option<ProducedTexture>, RasterizeFailure> {
        if self.reject.replace(false) {
            // Explicit controlled error injection at the actual producer ABI:
            // this qualifies refusal handling, not provider limit detection.
            Err(RasterizeFailure::ProviderError(Arc::new(
                VelloError::CurveQualityReadbackFailed,
            )))
        } else {
            self.producer.render_checked(cx)
        }
    }
    fn suspend(&mut self) {
        self.producer.suspend();
    }
    fn portable_scene(
        &mut self,
        frame: &ProducerFrameInfo,
    ) -> Result<ProducedScene, PortableProducerError> {
        self.producer.portable_scene(frame)
    }
}
fn host() -> (
    TestHost,
    Arc<RenderCore>,
    Rc<RefCell<Source>>,
    Rc<Cell<bool>>,
    Arc<AtomicUsize>,
) {
    let core = Arc::new(
        RenderCore::boot(NetrenderOptions {
            tile_cache_size: Some(8),
            enable_vello: true,
            ..Default::default()
        })
        .expect("actual adapter required for ignored producer-refusal gate"),
    );
    let attempts = Arc::new(AtomicUsize::new(0));
    let counted = attempts.clone();
    let owner = SharedRenderCore::with_factory(
        core.clone(),
        Arc::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            Err("bounded replacement refusal for in-frame destruction control".into())
        }),
    );
    let source = Rc::new(RefCell::new(Source {
        revision: 1,
        destroy: false,
        refuse: false,
        frames: 0,
        core: Arc::downgrade(&core),
    }));
    let reject = Rc::new(Cell::new(false));
    let mut state = HostState::new();
    state.window = Some(Box::new(Window));
    state.surface = Some(Box::new(HeadlessSurface(core.clone())));
    state.shared.render_core = Some(core.clone());
    state.shared.sheet =
        format!("custom-leaf {{ display:block;width:{WIDTH}px;height:{HEIGHT}px; }}");
    state.runner = Some(Runner::new(
        Rc::new(RefCell::new(ScriptedDom::new())),
        root as Logic,
        State {
            draft: "pending authored draft".into(),
            source: source.clone(),
        },
    ));
    state
        .shared
        .producers
        .register(
            KEY,
            RejectNext {
                producer: SceneProducer::new_checked(
                    source.clone(),
                    0x7072_6f64_7265,
                    checked_frame,
                    revision,
                    |_| None,
                ),
                reject: reject.clone(),
            },
            &[],
        )
        .unwrap();
    let wake = HostWake::new(state.wake_pending.clone(), Arc::new(|| {}));
    let mut host = Host::new(
        HostOptions::default(),
        None,
        HostHooks::inert(),
        state,
        wake,
    );
    host.attach_shared_recovery(owner);
    host.set_surface_size(WIDTH as f32, HEIGHT as f32);
    (host, core, source, reject, attempts)
}
#[test]
#[ignore = "actual GPU staging; coordinator executes serialized gate"]
fn checked_scene_source_refuses_all_captures_and_recovers_without_replacement() {
    let (mut host, core, source, _, attempts) = host();
    host.redraw();
    assert_eq!(source.borrow().frames, 1);
    let generation = core.generation();
    let captures = arm(&mut host);
    source.borrow_mut().revision += 1;
    source.borrow_mut().refuse = true;
    host.redraw();
    assert_eq!(source.borrow().frames, 1, "refused source cannot paint");
    assert_eq!(
        host.s.shared.producers.error(KEY),
        Some(ProducerError::RenderRefused)
    );
    assert!(
        !core
            .renderer()
            .unregister_external_image(external_image_key(KEY)),
        "the prior staged image must be removed on refusal"
    );
    assert_eq!(captures.borrow().len(), 3);
    for refusal in captures.borrow().iter() {
        assert!(matches!(refusal, CaptureRefusal::PresentationError {
            source_key: Some(KEY), error, ..
        } if error.contains("authored scene refused")));
    }
    assert!(
        matches!(host.poll_device_recovery(), RecoveryStatus::Ready { generation: g } if g == generation)
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    assert_authority(&host, &source);
    source.borrow_mut().refuse = false;
    host.redraw();
    assert_eq!(source.borrow().frames, 2);
    assert_eq!(
        host.s.last_frame_profile.as_ref().unwrap().producers.stages,
        1
    );
}
fn arm(host: &mut TestHost) -> Rc<RefCell<Vec<CaptureRefusal>>> {
    let out = Rc::new(RefCell::new(Vec::new()));
    let image = out.clone();
    host.s.pending_capture = Some(Box::new(move |result| {
        image.borrow_mut().push(result.err().unwrap())
    }));
    let stamped = out.clone();
    host.s.pending_stamped_capture = Some(Box::new(move |result| {
        stamped.borrow_mut().push(result.err().unwrap())
    }));
    let paint = out.clone();
    host.s.pending_paint_capture = Some(Box::new(move |result| {
        paint.borrow_mut().push(result.err().unwrap())
    }));
    out
}
fn assert_authority(host: &TestHost, source: &Rc<RefCell<Source>>) {
    let state = host.s.runner.as_ref().unwrap().state();
    assert_eq!(state.draft, "pending authored draft");
    assert!(Rc::ptr_eq(&state.source, source));
    assert!(host.s.shared.producers.contains(KEY));
}
#[test]
#[ignore = "actual GPU staging; coordinator executes serialized gate"]
fn healthy_provider_refusal_drops_staged_image_without_booting_replacement() {
    let (mut host, core, source, reject, attempts) = host();
    host.redraw();
    assert_eq!(
        host.s.last_frame_profile.as_ref().unwrap().producers.stages,
        1
    );
    let generation = core.generation();
    let captures = arm(&mut host);
    reject.set(true);
    host.redraw();
    assert_eq!(
        host.s.shared.producers.error(KEY),
        Some(ProducerError::RenderRefused)
    );
    assert_eq!(
        host.s.shared.producers.render_failure().unwrap().source_key,
        KEY
    );
    assert!(
        !core
            .renderer()
            .unregister_external_image(external_image_key(KEY)),
        "the failed producer's previously staged image must already be gone"
    );
    assert_eq!(captures.borrow().len(), 3);
    for refusal in captures.borrow().iter() {
        assert!(matches!(refusal, CaptureRefusal::ProviderError {
            generation: actual, source_key: Some(KEY), error,
        } if *actual == generation && matches!(&**error, VelloError::CurveQualityReadbackFailed)));
    }
    assert!(
        matches!(host.poll_device_recovery(), RecoveryStatus::Ready { generation: g } if g == generation)
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    assert!(core.device_failure().is_none());
    assert_authority(&host, &source);
    host.redraw();
    assert_eq!(
        captures.borrow().len(),
        3,
        "armed requests must be refused exactly once"
    );
    assert_eq!(
        host.s.last_frame_profile.as_ref().unwrap().producers.stages,
        1,
        "the same bound source must rehydrate after the healthy refusal"
    );
}
#[test]
#[ignore = "actual device destruction during production frame; coordinator executes serialized gate"]
fn actual_scene_callback_destroys_device_after_host_preflight_without_panicking() {
    let (mut host, core, source, _, attempts) = host();
    host.redraw();
    assert_eq!(source.borrow().frames, 1);
    let captures = arm(&mut host);
    source.borrow_mut().revision += 1;
    source.borrow_mut().destroy = true;
    host.redraw();
    assert_eq!(
        source.borrow().frames,
        2,
        "the actual SceneProducer callback must execute during redraw"
    );
    assert_eq!(captures.borrow().len(), 3);
    let loss = core
        .loss_notice()
        .expect("actual destroy callback must arrive");
    assert_eq!(loss.reason, wgpu::DeviceLostReason::Destroyed);
    for refusal in captures.borrow().iter() {
        assert!(matches!(refusal, CaptureRefusal::DeviceLost(actual) if actual == &loss));
    }
    assert!(matches!(
        host.s.recovery_status,
        Some(RecoveryStatus::Failed { .. })
    ));
    assert!(host.s.surface.is_none() && host.s.shared.render_core.is_none());
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_authority(&host, &source);
    host.poll_device_recovery();
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(captures.borrow().len(), 3);
}
