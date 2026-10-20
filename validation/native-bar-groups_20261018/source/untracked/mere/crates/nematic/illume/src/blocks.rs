// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! Source-bound Djot div parsing and opening-declaration edits.
//!
//! This is a pure, wasm-safe source utility. It knows no domain rules and
//! never reformats a div body or grants declarations evaluation authority.

use jotdown::{Container, Event, Parser};
use std::{collections::BTreeMap, ops::Range};

/// A source-bound div. Ranges include its attribute/opening and closing fences;
/// the body range excludes them. Nested members keep their own source ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Div {
    pub range: Range<usize>,
    pub body: Range<usize>,
    pub attributes: BTreeMap<String, String>,
    pub children: Vec<Div>,
}

/// Source structure, independent of domain rules or the current presentation.
pub fn divs(source: &str) -> Vec<Div> {
    let mut roots = Vec::new();
    let mut stack: Vec<Div> = Vec::new();
    let mut pending = None;
    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Attributes(_) => pending = Some(range.start),
            Event::Start(Container::Div { class }, attrs) => {
                let mut attributes: BTreeMap<_, _> = attrs
                    .unique_pairs()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect();
                if !class.is_empty() {
                    let classes = attributes.entry("class".into()).or_default();
                    if !classes
                        .split_whitespace()
                        .any(|value| value == class.as_ref())
                    {
                        if !classes.is_empty() {
                            classes.push(' ');
                        }
                        classes.push_str(&class);
                    }
                }
                stack.push(Div {
                    range: pending.take().unwrap_or(range.start)..source.len(),
                    body: range.end..source.len(),
                    attributes,
                    children: Vec::new(),
                });
            },
            Event::End(Container::Div { .. }) => {
                if let Some(mut div) = stack.pop() {
                    div.body.end = range.start.max(div.body.start);
                    div.range.end = range.end.max(div.body.end);
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(div);
                    } else {
                        roots.push(div);
                    }
                }
                pending = None;
            },
            _ => pending = None,
        }
    }
    roots
}

/// Replace one div's complete attribute declaration while preserving every
/// other source byte. `target` must be an exact node returned by [`divs`] for
/// `source`; this keeps a caller from using a stale range to edit unrelated
/// text. Existing declarations are replaced in place. When a div has no
/// declaration, a prefixed attribute line is inserted immediately before its
/// opening fence, which also works inside block quotes and lists.
pub fn update_opening(
    source: &str,
    target: &Div,
    attributes: &BTreeMap<String, String>,
) -> Result<String, String> {
    fn contains(nodes: &[Div], wanted: &Div) -> bool {
        nodes
            .iter()
            .any(|node| node == wanted || contains(&node.children, wanted))
    }

    if !contains(&divs(source), target) {
        return Err("target div does not match the current source".into());
    }
    validate_attributes(attributes)?;

    let declaration = if attributes.is_empty() {
        String::new()
    } else {
        let mut out = String::from("{");
        for (index, (key, value)) in attributes.iter().enumerate() {
            if index > 0 {
                out.push(' ');
            }
            out.push_str(key);
            out.push_str("=\"");
            out.push_str(&value.replace('\\', "\\\\").replace('"', "\\\""));
            out.push('"');
        }
        out.push('}');
        out
    };

    let open_end = target.body.start;
    if open_end > source.len() || !source.is_char_boundary(open_end) {
        return Err("target div has an invalid opening range".into());
    }
    let prefix_source = &source[..open_end];
    let fence_line_start = prefix_source
        .strip_suffix('\n')
        .and_then(|prefix| prefix.rfind('\n').map(|at| at + 1))
        .unwrap_or(0);
    let fence_line = &source[fence_line_start..open_end];
    let fence_offset = fence_line
        .find(':')
        .ok_or_else(|| "target div opening fence was not found".to_string())?;
    let line_prefix = &fence_line[..fence_offset];
    let (attribute_prefix, fence_prefix) = list_prefixes(line_prefix);

    // Use jotdown's token range for the actual attribute event. Searching for
    // braces in source is unsafe because valid quoted values (for example a
    // JSON attribute) can contain both braces and escaped quotes.
    let declaration_range = Parser::new(source)
        .into_offset_iter()
        .find_map(|(event, range)| match event {
            Event::Attributes(_) if range.start == target.range.start => Some((range, false)),
            // In block quotes and lists jotdown can fold a standalone
            // declaration into the following div's Start event. Its offset
            // starts at the declaration token, while `target.range.start`
            // keeps the same token offset.
            Event::Start(Container::Div { .. }, attrs)
                if range.start == target.range.start
                    && range.start < fence_line_start
                    && attrs.unique_pairs().next().is_some() =>
            {
                Some((range.start..fence_line_start + line_prefix.len(), true))
            },
            _ => None,
        });
    if let Some((range, includes_fence_prefix)) = declaration_range {
        if range.start < target.range.start
            || range.end > open_end
            || !source.is_char_boundary(range.start)
            || !source.is_char_boundary(range.end)
        {
            return Err("target div has an invalid attribute range".into());
        }
        let mut updated = String::with_capacity(source.len() + declaration.len());
        updated.push_str(&source[..range.start]);
        updated.push_str(&declaration);
        if includes_fence_prefix {
            updated.push('\n');
            updated.push_str(line_prefix);
        }
        updated.push_str(&source[range.end..]);
        return Ok(updated);
    }

    if declaration.is_empty() {
        return Ok(source.to_string());
    }
    let mut updated =
        String::with_capacity(source.len() + declaration.len() + attribute_prefix.len() + 1);
    updated.push_str(&source[..fence_line_start]);
    updated.push_str(&attribute_prefix);
    updated.push_str(&declaration);
    updated.push('\n');
    updated.push_str(&fence_prefix);
    updated.push_str(&source[fence_line_start + line_prefix.len()..]);
    Ok(updated)
}

/// Validate a complete single-line Djot attribute map before source mutation.
pub fn validate_attributes(attributes: &BTreeMap<String, String>) -> Result<(), String> {
    for (key, value) in attributes {
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_:-".contains(&byte))
            || value.contains(['\n', '\r', '\0'])
        {
            return Err("div attributes must have valid keys and single-line values".into());
        }
    }
    Ok(())
}

/// Turn a list item's marker into a same-width continuation indentation for
/// the opening fence. The attribute line retains the original marker, keeping
/// the div inside that list item rather than starting a sibling item.
fn list_prefixes(prefix: &str) -> (String, String) {
    let quote_end = prefix
        .rfind('>')
        .map(|position| {
            let after = position + 1;
            after + usize::from(prefix[after..].starts_with(' '))
        })
        .unwrap_or(0);
    let quote = &prefix[..quote_end];
    let rest = &prefix[quote_end..];
    let marker_start = rest
        .bytes()
        .position(|byte| byte != b' ')
        .unwrap_or(rest.len());
    let marker = rest[marker_start..].trim_end_matches(' ');
    let is_marker = marker == "-"
        || marker == "+"
        || marker == "*"
        || marker
            .strip_suffix('.')
            .or_else(|| marker.strip_suffix(')'))
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
    if !is_marker
        || !rest[marker_start + marker.len()..]
            .bytes()
            .all(|byte| byte == b' ')
    {
        return (prefix.to_owned(), prefix.to_owned());
    }
    let mut continuation = quote.to_owned();
    continuation.push_str(&rest[..marker_start]);
    continuation.push_str(&" ".repeat(rest.len() - marker_start));
    (prefix.to_owned(), continuation)
}

/// Wrap authored source without parsing or reformatting its contents.
pub fn wrap(attributes: &BTreeMap<String, String>, body: &str) -> String {
    let fence = ":".repeat(
        body.lines()
            .map(|line| line.trim_start().chars().take_while(|c| *c == ':').count())
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(3),
    );
    let mut out = String::new();
    if !attributes.is_empty() {
        out.push('{');
        for (i, (key, value)) in attributes.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(key);
            out.push_str("=\"");
            out.push_str(&value.replace('\\', "\\\\").replace('"', "\\\""));
            out.push('"');
        }
        out.push_str("}\n");
    }
    out.push_str(&fence);
    out.push('\n');
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_divs_keep_exact_source_ranges() {
        let source = "before\n\n{#outer owner=human}\n:::: outer\ninside α\n\n{#inner keep=yes}\n::: child\nbody 漢\n:::\n::::\nafter\n";
        let tree = divs(source);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(&source[tree[0].children[0].body.clone()], "body 漢\n");
    }

    #[test]
    fn update_uses_parser_range_for_json_braces_and_keeps_body_and_unknown_attrs() {
        let source = r#"{#card json="{\"quote\":\"a } { value\"}" custom="keep"}
::: panel
body { bytes } α
:::
"#;
        let target = divs(source).remove(0);
        let mut attributes = target.attributes.clone();
        attributes.insert("status".into(), "ready".into());
        let updated = update_opening(source, &target, &attributes).unwrap();
        let updated_target = divs(&updated).remove(0);
        assert_eq!(updated_target.attributes, attributes);
        assert_eq!(&updated[updated_target.body.clone()], "body { bytes } α\n");
        assert!(updated.contains("custom=\"keep\""));
    }

    #[test]
    fn update_inserts_prefixed_attrs_without_rewriting_quote_body() {
        let source = "> ::: panel\n> α\n> :::\n";
        let target = divs(source).remove(0);
        let updated = update_opening(
            source,
            &target,
            &BTreeMap::from([("label".into(), "quoted".into())]),
        )
        .unwrap();
        assert!(updated.starts_with("> {label=\"quoted\"}\n> ::: panel\n"));
        assert!(updated.ends_with("> α\n> :::\n"));
    }

    #[test]
    fn update_keeps_a_list_div_inside_its_parent_item() {
        let source = "- ::: panel\n  α\n  :::\n";
        let target = divs(source).remove(0);
        let updated = update_opening(
            source,
            &target,
            &BTreeMap::from([("label".into(), "item".into())]),
        )
        .unwrap();
        assert!(updated.starts_with("- {label=\"item\"}\n  ::: panel\n"));
        assert!(updated.ends_with("  α\n  :::\n"));
        assert_eq!(divs(&updated).len(), 1);
    }

    #[test]
    fn update_rejects_forged_target_and_multiline_attributes() {
        let source = "::: panel\nα\n:::\n";
        let target = divs(source).remove(0);
        let mut forged = target.clone();
        forged.attributes.insert("id".into(), "other".into());
        assert!(update_opening(source, &forged, &BTreeMap::new()).is_err());
        assert!(
            update_opening(
                source,
                &target,
                &BTreeMap::from([("label".into(), "one\ntwo".into())]),
            )
            .is_err()
        );
    }
}
