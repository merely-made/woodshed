/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The file seam through the host's own routing: a view's request reaches the
//! host's chooser after the click that made it, and the chooser's answer
//! comes back to that view.

use std::cell::RefCell;
use std::rc::Rc;

use cambium::{
    AnyView, FileEvent, FileFilter, FileRequest, GenetCtx, GenetElement, OpenedFile, PointerClick,
    button, el, open_file,
};
use cambium_genet_winit_host::Harness;
use cambium_rootstock::{FileAnswer, FileChooser};
use genet_scripted_dom::{NodeId, ScriptedDom};
use layout_dom_api::{LayoutDom, LocalName, Namespace};
use taproot::Selector;

#[derive(Default)]
struct Page {
    asking: bool,
    answers: usize,
    opened: Vec<String>,
}

type Child = Box<dyn AnyView<Page, (), GenetCtx, GenetElement>>;
type Logic = fn(&Page) -> Child;

fn page(page: &Page) -> Child {
    Box::new(el(
        "main",
        open_file(
            button("Open", |page: &mut Page, _: PointerClick| {
                page.asking = true
            })
            .attr("id", "open")
            .attr("class", "open"),
            page.asking,
            FileFilter::extensions(["txt"]),
            |page: &mut Page, event: FileEvent| {
                page.asking = false;
                page.answers += 1;
                page.opened = event.files.into_iter().map(|file| file.name).collect();
            },
        ),
    ))
}

const SHEET: &str = "button { width: 80px; height: 24px; }";

fn laid_out() -> Harness<Page, Logic, Child> {
    let mut h = Harness::new(SHEET, Page::default(), page as Logic);
    h.layout_at(400.0, 300.0);
    h
}

fn file(name: &str) -> OpenedFile {
    OpenedFile {
        name: name.into(),
        media_type: Some("text/plain".into()),
        last_modified_ms: None,
        bytes: b"notes".to_vec(),
    }
}

/// A test's chooser: it records each request and answers with its file, or
/// leaves the request unanswered when it has none.
struct Prepared {
    asked: Rc<RefCell<Vec<FileRequest>>>,
    file: Option<OpenedFile>,
}

impl FileChooser for Prepared {
    fn open(&mut self, request: &FileRequest, answer: FileAnswer) {
        self.asked.borrow_mut().push(request.clone());
        if let Some(file) = &self.file {
            answer.send(FileEvent {
                files: vec![file.clone()],
            });
        }
    }
}

fn button_node(h: &Harness<Page, Logic, Child>) -> NodeId {
    fn find(dom: &ScriptedDom, node: NodeId) -> Option<NodeId> {
        if dom.attribute(node, &Namespace::from(""), &LocalName::from("id")) == Some("open") {
            return Some(node);
        }
        dom.dom_children(node).find_map(|child| find(dom, child))
    }
    h.with_dom(|dom| find(dom, dom.document()))
        .expect("the page has its button")
}

#[test]
fn a_click_asks_the_chooser_and_its_answer_reaches_the_view() {
    let mut h = laid_out();
    let asked = Rc::new(RefCell::new(Vec::new()));
    h.set_file_chooser(Box::new(Prepared {
        asked: asked.clone(),
        file: Some(file("notes.txt")),
    }));
    assert!(h.click_on(&Selector::class("open")));
    let requests = asked.borrow().clone();
    assert_eq!(
        requests,
        [FileRequest {
            node: button_node(&h),
            filter: FileFilter::extensions(["txt"]),
        }]
    );
    assert_eq!(h.state().answers, 0, "the answer waits for a frame");
    assert!(h.deliver_files());
    assert_eq!(h.state().opened, ["notes.txt"]);
    assert!(!h.state().asking);
}

#[test]
fn a_request_held_open_is_not_asked_again() {
    let mut h = laid_out();
    let asked = Rc::new(RefCell::new(Vec::new()));
    h.set_file_chooser(Box::new(Prepared {
        asked: asked.clone(),
        file: None,
    }));
    assert!(h.click_on(&Selector::class("open")));
    assert!(h.click_on(&Selector::class("open")));
    assert_eq!(asked.borrow().len(), 1);
    assert!(h.state().asking, "no answer came");
}

#[test]
fn with_no_chooser_the_view_hears_that_nothing_was_chosen() {
    let mut h = laid_out();
    assert!(h.click_on(&Selector::class("open")));
    assert!(h.deliver_files());
    assert_eq!(
        (h.state().answers, h.state().opened.len(), h.state().asking),
        (1, 0, false)
    );
}

#[test]
fn a_file_is_supplied_without_a_chooser() {
    let mut h = laid_out();
    let node = button_node(&h);
    h.supply_files(
        node,
        FileEvent {
            files: vec![file("data.csv")],
        },
    );
    assert_eq!(h.state().opened, ["data.csv"]);
}
