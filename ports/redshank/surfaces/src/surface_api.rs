//! Mounting the compact dock inside a second host.
//!
//! The standalone host owns its runner and calls `drain_commands` on its own
//! state each frame. An embedding host admits the dock through Mere's
//! contributed-surface seam, which erases the runner behind
//! [`RetainedSurfaceSession`]; that trait carries no state access at all, so
//! the two facts a host must exchange every frame — the projection in, the
//! commands out — travel through a shared [`CompactDock`] handle instead.
//!
//! This is why the session is a small [`RetainedSurfaceSession`] of its own
//! rather than a `RunnerSurfaceSession`: that wrapper's action type is `()`
//! (the dock queues commands in its state, it does not bubble them) and its
//! viewport hook fires only when the viewport changes, so neither of its two
//! product closures can carry a per-frame pump. Everything else — the
//! fourteen erased methods — is the same one-line delegation.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use cambium::{
    DomHandle, GenetAppRunner, ResolvedSurfaceEvent, RetainedSurfaceSession, SurfaceEffect,
    SurfaceViewport, el,
};
use genet_scripted_dom::NodeId;
use mere_surface_api::{
    ProviderId, SourceKindId, SurfaceAvailability, SurfaceDescriptor, SurfaceId,
    SurfaceSourceShape, SurfaceUnavailableReason,
};

use crate::theme::scope_class;
use crate::{
    CompactCommand, CompactPlayerState, CompactView, Mode, Seed, TransportState, compact_surface,
};

/// Redshank's provider identity in a host's surface registry.
pub const PROVIDER_ID: &str = "redshank";
/// The compact Player/Capture dock's surface identity.
pub const COMPACT_SURFACE_ID: &str = "redshank.compact";
/// The versioned source kind an embedding host's episode payload must name.
/// A host's own schema id has to equal this or its registry refuses the
/// provider: the descriptor is the stated admission truth.
pub const EPISODE_SOURCE_KIND: &str = "redshank.episode.v1";

/// The wrapper the hosted dock's root carries, so a host can size it.
pub const HOSTED_DOCK_CSS: &str = ".rs-hosted-dock { justify-content: flex-start; padding: 0; }\n\
     .rs-hosted-dock .rs-dock { border-top: none; }\n";

/// Stable data-only descriptor for the compact listening dock.
pub fn compact_descriptor() -> SurfaceDescriptor {
    SurfaceDescriptor {
        provider_id: ProviderId::from(PROVIDER_ID),
        surface_id: SurfaceId::from(COMPACT_SURFACE_ID),
        label: "Redshank listening dock".to_owned(),
        accepted_source: SurfaceSourceShape::One(SourceKindId::from(EPISODE_SOURCE_KIND)),
    }
}

/// The stylesheet an embedding host must lay the dock out under.
pub fn compact_stylesheet() -> String {
    let mut sheet = crate::theme::sheet();
    sheet.push_str(HOSTED_DOCK_CSS);
    sheet
}

/// The host's half of one mounted dock: it writes the projection and drains
/// the commands. Clones share one dock, so the host keeps a clone and the
/// admitted session keeps the other.
#[derive(Clone, Default)]
pub struct CompactDock {
    projection: Rc<RefCell<CompactPlayerState>>,
    commands: Rc<RefCell<VecDeque<CompactCommand>>>,
}

impl CompactDock {
    pub fn new(state: CompactPlayerState) -> Self {
        Self {
            projection: Rc::new(RefCell::new(state)),
            commands: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    /// Replace what the dock shows. The session applies it on the next frame.
    pub fn project(&self, state: CompactPlayerState) {
        *self.projection.borrow_mut() = state;
    }

    /// What the host last projected.
    pub fn projection(&self) -> CompactPlayerState {
        self.projection.borrow().clone()
    }

    /// Queue a command as the dock itself would. Hosts use this for the
    /// affordances they supply beside the dock (a note editor, say).
    pub fn request(&self, command: CompactCommand) {
        self.commands.borrow_mut().push_back(command);
    }

    /// Take every command the listener has issued since the last drain.
    pub fn drain(&self) -> Vec<CompactCommand> {
        self.commands.borrow_mut().drain(..).collect()
    }

    pub fn pending(&self) -> usize {
        self.commands.borrow().len()
    }
}

type DockRunner<Logic> = GenetAppRunner<CompactPlayerState, Logic, CompactView, ()>;

/// One admitted compact dock, erased for a host's retained-surface registry.
struct CompactDockSession<Logic>
where
    Logic: FnMut(&CompactPlayerState) -> CompactView,
{
    descriptor: SurfaceDescriptor,
    runner: DockRunner<Logic>,
    dock: CompactDock,
    viewport: Option<SurfaceViewport>,
}

impl<Logic> CompactDockSession<Logic>
where
    Logic: FnMut(&CompactPlayerState) -> CompactView,
{
    /// Move queued commands out to the host and pull the host's newest
    /// projection in. The dock keeps no state the host does not own, so the
    /// projection simply replaces it.
    fn pump(&mut self) -> Vec<SurfaceEffect> {
        if *self.runner.state() == *self.dock.projection.borrow() {
            return Vec::new();
        }
        let dock = self.dock.clone();
        self.runner.update(move |state| {
            let queued: Vec<CompactCommand> = state.drain_commands().collect();
            dock.commands.borrow_mut().extend(queued);
            *state = dock.projection.borrow().clone();
        });
        vec![SurfaceEffect::Redraw]
    }
}

impl<Logic> RetainedSurfaceSession for CompactDockSession<Logic>
where
    Logic: FnMut(&CompactPlayerState) -> CompactView,
{
    fn descriptor(&self) -> &SurfaceDescriptor {
        &self.descriptor
    }

    fn availability(&self) -> SurfaceAvailability {
        match &self.runner.state().transport {
            TransportState::Unavailable(message) => {
                SurfaceAvailability::Unavailable(SurfaceUnavailableReason::Other(message.clone()))
            },
            _ => SurfaceAvailability::Available,
        }
    }

    fn dom(&self) -> DomHandle {
        self.runner.dom()
    }

    fn root(&self) -> NodeId {
        self.runner.root()
    }

    fn focus(&self) -> Option<NodeId> {
        self.runner.focus()
    }

    fn set_focus(&mut self, node: Option<NodeId>) -> Vec<SurfaceEffect> {
        self.runner.set_focus(node);
        vec![SurfaceEffect::Redraw]
    }

    fn focus_traverse(&mut self, forward: bool) -> Vec<SurfaceEffect> {
        self.runner.focus_traverse(forward);
        vec![SurfaceEffect::Redraw]
    }

    fn focusables(&self) -> Vec<NodeId> {
        self.runner.focusables()
    }

    fn pointer_capture(&self) -> Option<NodeId> {
        self.runner.pointer_capture()
    }

    fn pointer_target(&self, hit: NodeId) -> Option<NodeId> {
        self.runner.pointer_target(hit)
    }

    fn hover_target(&self, hit: NodeId) -> Option<NodeId> {
        self.runner.hover_target(hit)
    }

    fn wheel_target(&self, hit: NodeId) -> Option<NodeId> {
        self.runner.wheel_target(hit)
    }

    /// The host's per-frame hook: it runs on every laid-out frame, not only
    /// on a resize, so the pump rides it.
    fn sync_viewport(&mut self, viewport: SurfaceViewport) -> Vec<SurfaceEffect> {
        let resized = self.viewport != Some(viewport);
        self.viewport = Some(viewport);
        let mut effects = self.pump();
        if resized && effects.is_empty() {
            effects.push(SurfaceEffect::Redraw);
        }
        effects
    }

    fn dispatch(&mut self, event: ResolvedSurfaceEvent) -> Vec<SurfaceEffect> {
        match event {
            ResolvedSurfaceEvent::Click { target, event } => {
                self.runner.dispatch_click(target, event)
            },
            ResolvedSurfaceEvent::Key(event) => self.runner.dispatch_key(event),
            ResolvedSurfaceEvent::PointerDown { target, event } => {
                self.runner.dispatch_pointer_down(target, event)
            },
            ResolvedSurfaceEvent::PointerMove(event) => self.runner.dispatch_pointer_move(event),
            ResolvedSurfaceEvent::PointerUp(event) => self.runner.dispatch_pointer_up(event),
            ResolvedSurfaceEvent::Hover { target, event } => {
                self.runner.dispatch_hover(target, event)
            },
            ResolvedSurfaceEvent::Wheel { target, event } => {
                self.runner.dispatch_wheel(target, event)
            },
        };
        let mut effects = vec![SurfaceEffect::Redraw];
        effects.extend(self.pump());
        effects
    }
}

/// Erase one compact dock behind Cambium's retained-session contract.
///
/// The seed and mode choose the scope class the dock's tokens resolve
/// against; the host's own chrome keeps its own theme.
pub fn compact_session(
    dom: DomHandle,
    dock: CompactDock,
    seed: Seed,
    mode: Mode,
) -> Box<dyn RetainedSurfaceSession> {
    let class = format!("rs-app rs-hosted-dock {}", scope_class(seed, mode));
    let logic = move |state: &CompactPlayerState| -> CompactView {
        Box::new(el("div", vec![compact_surface(state)]).attr("class", class.clone()))
    };
    let state = dock.projection();
    let runner = GenetAppRunner::new(dom, logic, state);
    Box::new(CompactDockSession {
        descriptor: compact_descriptor(),
        runner,
        dock,
        viewport: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Face, NowPlaying, SourceKind};
    use cambium::PointerClick;
    use genet_scripted_dom::ScriptedDom;
    use layout_dom_api::{LayoutDom, LocalName, Namespace};
    use redshank_model::ItemId;

    fn dom() -> DomHandle {
        Rc::new(RefCell::new(ScriptedDom::new()))
    }

    fn playing() -> CompactPlayerState {
        CompactPlayerState {
            transport: TransportState::Playing,
            now_playing: Some(NowPlaying {
                item_id: ItemId("episode-1".into()),
                title: "Wetland".into(),
                feed_title: Some("Marsh Notes".into()),
                face: Face::Tag("mp3".into()),
                source: SourceKind::Cloud,
                position_ms: 1_000,
                duration_ms: Some(60_000),
                resumed_from_ms: None,
                buffered_percent: 50,
                markers: Vec::new(),
            }),
            ..CompactPlayerState::default()
        }
    }

    fn labelled(dom: &ScriptedDom, root: NodeId, label: &str) -> NodeId {
        let aria = LocalName::from("aria-label");
        let empty = Namespace::from("");
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if dom
                .attribute(node, &empty, &aria)
                .is_some_and(|value| value == label)
            {
                return node;
            }
            pending.extend(dom.dom_children(node));
        }
        panic!("missing control {label}");
    }

    #[test]
    fn the_descriptor_accepts_the_episode_source_kind() {
        let descriptor = compact_descriptor();
        assert_eq!(descriptor.provider_id.as_str(), PROVIDER_ID);
        assert_eq!(descriptor.surface_id.as_str(), COMPACT_SURFACE_ID);
        assert_eq!(descriptor.label, "Redshank listening dock");
        let SurfaceSourceShape::One(kind) = &descriptor.accepted_source else {
            panic!("the dock accepts exactly one source");
        };
        assert_eq!(kind.as_str(), EPISODE_SOURCE_KIND);
    }

    #[test]
    fn the_session_renders_the_dock_under_the_scope_class() {
        let dock = CompactDock::new(playing());
        let session = compact_session(dom(), dock, Seed::Wetland, Mode::Dark);
        let markup = session.dom().borrow().outer_html(session.root());
        assert!(markup.contains("rs-app rs-hosted-dock t-redshank"));
        assert!(markup.contains("Wetland"));
        assert!(markup.contains("aria-label=\"Pause\""));
        assert!(session.availability().is_available());
    }

    #[test]
    fn a_pressed_control_reaches_the_host_through_the_dock_handle() {
        let dock = CompactDock::new(playing());
        let mut session = compact_session(dom(), dock.clone(), Seed::Wetland, Mode::Dark);
        let pause = labelled(&session.dom().borrow(), session.root(), "Pause");
        session.dispatch(ResolvedSurfaceEvent::Click {
            target: pause,
            event: PointerClick::at((1.0, 1.0)),
        });
        assert_eq!(dock.drain(), [CompactCommand::Pause]);
        assert_eq!(dock.drain(), []);
    }

    #[test]
    fn a_host_projection_reaches_the_dock_on_the_next_frame() {
        let dock = CompactDock::new(playing());
        let mut session = compact_session(dom(), dock.clone(), Seed::Wetland, Mode::Dark);
        let mut next = playing();
        if let Some(now) = next.now_playing.as_mut() {
            now.position_ms = 42_000;
        }
        next.transport = TransportState::Paused;
        dock.project(next);
        let effects = session.sync_viewport(SurfaceViewport {
            width: 640.0,
            height: 120.0,
            scale_factor: 1.0,
        });
        assert!(effects.contains(&SurfaceEffect::Redraw));
        let markup = session.dom().borrow().outer_html(session.root());
        assert!(markup.contains("0:42"));
        assert!(markup.contains("aria-label=\"Play\""));
    }

    #[test]
    fn an_unavailable_transport_reports_itself_to_the_host() {
        let mut state = playing();
        state.transport = TransportState::Unavailable("Output device unavailable".into());
        let dock = CompactDock::new(state);
        let session = compact_session(dom(), dock, Seed::Wetland, Mode::Dark);
        assert!(!session.availability().is_available());
    }
}
