//! Every annotation Redshank exports passes the W3C Web Annotation test
//! suite's MUST assertions. The assertions and the schema definitions they
//! reference are vendored under `tests/fixtures/w3c-web-annotation/`; see its
//! README for provenance and licence.
//!
//! The instrument is proved in the same run: the specification's own example
//! passes every assertion, and an annotation with a malformed `created` fails
//! exactly the assertion that guards it.

use std::{collections::HashMap, fs, path::Path, sync::Arc};

use jsonschema::{Draft, Retrieve, Uri, Validator};
use redshank_model::{
    Annotation, AnnotationId, CaptureAnchor, ItemId, LibraryItem, MediaSource, NoteBody,
    NotePrivacy, RedshankModel, RepresentationReceipt,
};
use serde_json::{Value, json};

const FIXTURES: &str = "tests/fixtures/w3c-web-annotation";

/// The suite's `$ref`s name a definitions file by its bare name, whatever
/// directory the referring assertion sits in.
struct Definitions(HashMap<String, Value>);

impl Retrieve for Definitions {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.path().as_str().rsplit('/').next().unwrap_or_default();
        self.0
            .get(name)
            .cloned()
            .ok_or_else(|| format!("no vendored definition for {uri}").into())
    }
}

struct Suite {
    /// `(relative path, compiled assertion, expected valid)`.
    assertions: Vec<(String, Validator, bool)>,
}

impl Suite {
    fn load() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES);
        let mut definitions = HashMap::new();
        for entry in fs::read_dir(root.join("definitions")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                definitions.insert(name, read_json(&path));
            }
        }
        let definitions = Arc::new(Definitions(definitions));
        let musts = read_json(&root.join("annotations/annotationMusts.test"));
        let assertions = musts["assertions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rel| {
                let rel = rel.as_str().unwrap().to_owned();
                let schema = read_json(&root.join(&rel));
                let definitions = Arc::clone(&definitions);
                let validator = jsonschema::options()
                    .with_draft(Draft::Draft4)
                    .should_validate_formats(true)
                    .with_base_uri("https://w3c.github.io/web-annotation-tests/annotations/")
                    .with_retriever(Shared(definitions))
                    .build(&schema)
                    .unwrap_or_else(|error| panic!("{rel}: {error}"));
                let expected = schema["expectedResult"] == "valid";
                (rel, validator, expected)
            })
            .collect();
        Self { assertions }
    }

    /// The assertions this annotation fails, with the suite's own message.
    fn failures(&self, annotation: &Value) -> Vec<String> {
        self.assertions
            .iter()
            .filter(|(_, validator, expected)| validator.is_valid(annotation) != *expected)
            .map(|(rel, validator, _)| {
                let detail = validator
                    .iter_errors(annotation)
                    .map(|error| error.to_string())
                    .next()
                    .unwrap_or_default();
                format!("{rel}: {detail}")
            })
            .collect()
    }
}

struct Shared(Arc<Definitions>);

impl Retrieve for Shared {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        self.0.retrieve(uri)
    }
}

fn read_json(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn anchor(item: &str, offset_ms: u64, end_offset_ms: Option<u64>) -> CaptureAnchor {
    CaptureAnchor {
        item_id: ItemId(item.into()),
        offset_ms,
        end_offset_ms,
        pressed_offset_ms: None,
        representation: RepresentationReceipt::default(),
    }
}

/// A library with every source kind the export names, and every body kind.
fn model() -> RedshankModel {
    let mut model = RedshankModel::default();
    model
        .add_item(LibraryItem::DirectAudio {
            id: ItemId("remote".into()),
            title: "Remote".into(),
            source: MediaSource::Enclosure {
                url: "https://example.test/a.mp3".into(),
            },
        })
        .unwrap();
    model
        .add_item(LibraryItem::LocalAudio {
            id: ItemId("local".into()),
            title: "Local".into(),
            source: MediaSource::Local {
                path: if cfg!(windows) {
                    r"C:\audio\talk one.mp3".into()
                } else {
                    "/audio/talk one.mp3".into()
                },
            },
        })
        .unwrap();
    model
        .add_item(LibraryItem::LocalAudio {
            id: ItemId("relative".into()),
            title: "Relative".into(),
            source: MediaSource::Local {
                path: "relative.mp3".into(),
            },
        })
        .unwrap();
    model
        .add_item(LibraryItem::DirectAudio {
            id: ItemId("blob".into()),
            title: "Blob".into(),
            source: MediaSource::HostBlob {
                id: "abc123".into(),
            },
        })
        .unwrap();
    model
        .add_text_annotation(
            AnnotationId("point".into()),
            anchor("remote", 12_500, None),
            "point".into(),
            1_758_900_000_000,
        )
        .unwrap();
    model
        .add_span_annotation(
            AnnotationId("span".into()),
            anchor("remote", 30_000, None),
            45_250,
            "span".into(),
            2,
        )
        .unwrap();
    model
        .add_text_annotation(
            AnnotationId("local".into()),
            anchor("local", 7, None),
            "local".into(),
            3,
        )
        .unwrap();
    model
        .add_text_annotation(
            AnnotationId("relative".into()),
            anchor("relative", 8, None),
            "relative".into(),
            4,
        )
        .unwrap();
    model
        .add_annotation(Annotation {
            id: AnnotationId("voice".into()),
            target: anchor("blob", 500, None),
            body: NoteBody::Audio {
                blob_id: "voice:abc".into(),
                media_type: "audio/wav".into(),
                duration_ms: 1_500,
            },
            created_at_ms: 5,
            privacy: NotePrivacy::Private,
        })
        .unwrap();
    for id in ["point", "span", "local", "relative", "voice"] {
        model
            .set_annotation_privacy(&AnnotationId(id.into()), NotePrivacy::Shareable)
            .unwrap();
    }
    model
}

#[test]
fn the_specifications_own_example_passes_every_must_assertion() {
    let suite = Suite::load();
    assert_eq!(suite.assertions.len(), 54);
    let example = json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "id": "http://example.org/anno1",
        "type": "Annotation",
        "body": "http://example.org/post1",
        "target": "http://example.com/page1"
    });
    assert_eq!(suite.failures(&example), Vec::<String>::new());
}

#[test]
fn a_malformed_created_fails_exactly_its_assertion() {
    let suite = Suite::load();
    let mut broken = json!({
        "@context": "http://www.w3.org/ns/anno.jsonld",
        "id": "http://example.org/anno1",
        "type": "Annotation",
        "created": "yesterday",
        "target": "http://example.com/page1"
    });
    let failures = suite.failures(&broken);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].starts_with("annotations/3.3.1-annotationCreatedValidated.json"));
    broken["created"] = json!("2026-09-26T16:00:00Z");
    assert_eq!(suite.failures(&broken), Vec::<String>::new());
}

#[test]
fn every_exported_annotation_passes_every_must_assertion() {
    let suite = Suite::load();
    let model = model();
    let mut exported = 0;
    for item in ["remote", "local", "relative", "blob"] {
        let page = model.export_annotations(&ItemId(item.into()));
        assert_eq!(page["@context"], "http://www.w3.org/ns/anno.jsonld");
        assert_eq!(page["type"], "AnnotationPage");
        for annotation in page["items"].as_array().unwrap() {
            exported += 1;
            let failures = suite.failures(annotation);
            assert!(
                failures.is_empty(),
                "{}\n{failures:#?}",
                serde_json::to_string_pretty(annotation).unwrap()
            );
        }
    }
    assert_eq!(exported, 5);
}
