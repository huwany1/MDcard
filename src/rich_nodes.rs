// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Lossless source spans; the transformed buffer is for CommonMark preview only.
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use std::{
    hash::{Hash, Hasher},
    ops::Range,
};

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: &'static str,
    pub range: Range<usize>,
    pub body: Range<usize>,
    pub source: String,
    pub raw: String,
    pub prefix: String,
    pub prefixes: Vec<String>,
    pub endings: Vec<String>,
    pub inline: bool,
}

pub fn hash(value: impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

pub fn parse(text: &str) -> (String, Vec<Node>) {
    let mut nodes = Vec::new();
    let mut excluded = Vec::<Range<usize>>::new();
    let mut block = None;
    // pulldown-cmark's fenced info-string handling requires LF. Replacing only
    // lone CR preserves every byte offset into the authoritative source.
    let parsed = normalize_lone_cr(text);
    for (event, range) in Parser::new(&parsed).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let engine = match kind {
                    CodeBlockKind::Fenced(s) => match s
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_lowercase()
                        .as_str()
                    {
                        "math" | "latex" => Some("math"),
                        "mermaid" => Some("mermaid"),
                        "plantuml" | "puml" => Some("plantuml"),
                        "flow" | "flowchart" => Some("flow"),
                        _ => None,
                    },
                    _ => None,
                };
                block = Some((range.start, engine, String::new()));
            }
            Event::Text(value) if block.is_some() => {
                block.as_mut().unwrap().2.push_str(&value);
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((start, engine, mut content)) = block.take() {
                    let whole = start..range.end;
                    excluded.push(whole.clone());
                    if let Some(kind) = engine {
                        let lines = line_spans(&text[whole.clone()]);
                        if lines.len() < 2 {
                            continue;
                        }
                        let (closing_start, closing_end, _) = *lines.last().unwrap();
                        let closing = &text[start + closing_start..start + closing_end];
                        let fence = text[start..].chars().next().unwrap_or('`');
                        let min_fence = text[start..].chars().take_while(|&c| c == fence).count();
                        if let Some(at) = closing.find(['`', '~']).filter(|&at| {
                            closing[at..].trim_end().chars().all(|c| c == fence)
                                && closing[at..].trim_end().len() >= min_fence
                        }) {
                            let prefix = closing[..at].to_string();
                            let first = start + lines[0].2;
                            let body_end = if lines.len() > 2 {
                                start + lines[lines.len() - 2].1
                            } else {
                                first
                            };
                            let body = first..body_end.max(first);
                            let raw = text[body.clone()].to_string();
                            if content.ends_with('\n') {
                                content.pop();
                            }
                            let source = content;
                            let source_lines = source.split('\n').collect::<Vec<_>>();
                            let mut prefixes = Vec::new();
                            let mut endings = Vec::new();
                            for (i, (a, b, c)) in line_spans(&raw).into_iter().enumerate() {
                                let rawline = &raw[a..b];
                                let semantic = source_lines.get(i).copied().unwrap_or("");
                                prefixes.push(
                                    rawline.strip_suffix(semantic).unwrap_or(&prefix).to_owned(),
                                );
                                endings.push(raw[b..c].to_string());
                            }
                            nodes.push(Node {
                                kind,
                                range: whole,
                                body,
                                source,
                                raw,
                                prefix,
                                prefixes,
                                endings,
                                inline: false,
                            });
                        }
                    }
                }
            }
            Event::Start(Tag::Link { .. }) => {
                let raw = &text[range.clone()];
                if raw.starts_with('[') {
                    let mut depth = 0;
                    let mut skip = false;
                    for (at, c) in raw.char_indices() {
                        if skip {
                            skip = false;
                            continue;
                        }
                        if c == '\\' {
                            skip = true;
                            continue;
                        }
                        if c == '[' {
                            depth += 1;
                        }
                        if c == ']' {
                            depth -= 1;
                            if depth == 0 {
                                excluded.push(range.start + at..range.end);
                                break;
                            }
                        }
                    }
                } else {
                    excluded.push(range);
                }
            }
            Event::Code(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::Start(Tag::Image { .. }) => excluded.push(range),
            _ => {}
        }
    }
    excluded.sort_by_key(|r| r.start);
    let mut merged = Vec::<Range<usize>>::new();
    for span in excluded {
        if let Some(last) = merged.last_mut().filter(|r| r.end >= span.start) {
            last.end = last.end.max(span.end);
        } else {
            merged.push(span);
        }
    }
    let excluded = merged;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(r) = covering(&excluded, i) {
            i = r.end;
            continue;
        }
        let escaped = |at: usize| {
            bytes[..at]
                .iter()
                .rev()
                .take_while(|&&b| b == b'\\')
                .count()
                % 2
                == 1
        };
        let (open, close, inline) = if text[i..].starts_with("\\(") && !escaped(i) {
            (2, "\\)", true)
        } else if text[i..].starts_with("\\[") && !escaped(i) {
            (2, "\\]", false)
        } else if bytes[i] == b'$' && !escaped(i) {
            if text[i..].starts_with("$$") {
                (2, "$$", false)
            } else if text
                .get(i + 1..)
                .and_then(|s| s.chars().next())
                .is_some_and(|c| !c.is_whitespace() && c != '$')
            {
                (1, "$", true)
            } else {
                i += 1;
                continue;
            }
        } else {
            i += text[i..].chars().next().unwrap().len_utf8();
            continue;
        };
        let start = i + open;
        let mut j = start;
        let mut found = None;
        while j < bytes.len() {
            if covering(&excluded, j).is_some() || (inline && matches!(bytes[j], b'\n' | b'\r')) {
                break;
            }
            if text[j..].starts_with(close) && !escaped(j) {
                let body = &text[start..j];
                let dollar = close == "$";
                if !body.is_empty()
                    && (!dollar
                        || (!body.ends_with(char::is_whitespace)
                            && !bytes.get(j + 1).is_some_and(u8::is_ascii_digit)))
                {
                    found = Some(j);
                    break;
                }
                // An invalid closing dollar cannot consume another equation later in a paragraph.
                break;
            }
            j += text[j..].chars().next().unwrap().len_utf8();
        }
        if let Some(j) = found {
            nodes.push(Node {
                kind: "math",
                range: i..j + close.len(),
                body: start..j,
                source: normalize(&text[start..j]),
                raw: text[start..j].to_owned(),
                prefix: String::new(),
                prefixes: Vec::new(),
                endings: line_spans(&text[start..j])
                    .iter()
                    .map(|&(_, b, c)| text[start + b..start + c].to_owned())
                    .collect(),
                inline,
            });
            i = j + close.len();
        } else {
            i = start;
        }
    }
    nodes.sort_by_key(|n| n.range.start);
    let mut preview = String::with_capacity(text.len());
    let mut cursor = 0;
    for (index, n) in nodes.iter().enumerate() {
        append_plain(&mut preview, text, cursor..n.range.start, &excluded);
        if n.inline {
            preview.push_str(&format!("$mdcard{index}$"));
        } else {
            preview.push_str(&format!("$$mdcard{index}$$"));
        }
        cursor = n.range.end;
    }
    append_plain(&mut preview, text, cursor..text.len(), &excluded);
    (normalize_lone_cr(&preview).into_owned(), nodes)
}
fn normalize_lone_cr(text: &str) -> std::borrow::Cow<'_, str> {
    let bytes = text.as_bytes();
    if !bytes
        .iter()
        .enumerate()
        .any(|(i, &b)| b == b'\r' && bytes.get(i + 1) != Some(&b'\n'))
    {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut normalized = bytes.to_vec();
    for i in 0..bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) != Some(&b'\n') {
            normalized[i] = b'\n';
        }
    }
    std::borrow::Cow::Owned(String::from_utf8(normalized).unwrap())
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
fn line_spans(text: &str) -> Vec<(usize, usize, usize)> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if matches!(bytes[i], b'\r' | b'\n') {
            let end = i;
            i += 1;
            if bytes[end] == b'\r' && bytes.get(i) == Some(&b'\n') {
                i += 1;
            }
            lines.push((start, end, i));
            start = i;
        } else {
            i += 1;
        }
    }
    if start < bytes.len() {
        lines.push((start, bytes.len(), bytes.len()));
    }
    lines
}

fn covering(excluded: &[Range<usize>], at: usize) -> Option<&Range<usize>> {
    let index = excluded.partition_point(|r| r.end <= at);
    excluded.get(index).filter(|r| r.contains(&at))
}
fn append_plain(out: &mut String, text: &str, span: Range<usize>, excluded: &[Range<usize>]) {
    for (off, c) in text[span.clone()].char_indices() {
        let at = span.start + off;
        if c == '$'
            && covering(excluded, at).is_none()
            && text.as_bytes()[..at]
                .iter()
                .rev()
                .take_while(|&&b| b == b'\\')
                .count()
                % 2
                == 0
        {
            out.push('\\');
        }
        out.push(c);
    }
}

pub fn patch(
    text: &str,
    expected: u64,
    node: &Node,
    replacement: &str,
) -> Result<String, &'static str> {
    if hash(text) != expected || text.get(node.body.clone()) != Some(node.raw.as_str()) {
        return Err("The source changed. Reopen this node.");
    }
    if replacement.len() > 64 * 1024 {
        return Err("The node exceeds the 64 KiB rendering limit.");
    }
    if replacement == node.source {
        return Ok(text.to_owned());
    }
    let replacement = normalize(replacement);
    let mut next = text.to_owned();
    let mut raw = String::new();
    let default = node
        .endings
        .iter()
        .find(|e| !e.is_empty())
        .map(String::as_str)
        .unwrap_or_else(|| {
            if text[node.range.clone()].contains("\r\n") {
                "\r\n"
            } else if text[node.range.clone()].contains('\r') {
                "\r"
            } else {
                "\n"
            }
        });
    let parts = replacement.split('\n').collect::<Vec<_>>();
    for (i, line) in parts.iter().enumerate() {
        raw.push_str(
            node.prefixes
                .get(i)
                .map(String::as_str)
                .unwrap_or(&node.prefix),
        );
        raw.push_str(line);
        if i + 1 < parts.len() {
            raw.push_str(
                node.endings
                    .get(i)
                    .filter(|e| !e.is_empty())
                    .map(String::as_str)
                    .unwrap_or(default),
            );
        }
    }
    if node.body.is_empty()
        && text[node.range.start..node.body.start]
            .trim_start()
            .starts_with("```")
        && text[node.body.end..node.range.end]
            .trim_start()
            .starts_with("```")
    {
        raw.push_str(default);
    }
    next.replace_range(node.body.clone(), &raw);
    let (_, after) = parse(&next);
    let valid = after
        .iter()
        .find(|n| n.range.start == node.range.start)
        .is_some_and(|n| {
            n.kind == node.kind
                && n.inline == node.inline
                && n.source == replacement
                && n.body.start == node.body.start
                && n.range.end == node.range.end - node.body.len() + raw.len()
        });
    if !valid {
        return Err("This edit changes the node delimiters. Edit it in the main source editor.");
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_fences_keep_container_prefixes() {
        for s in [
            "> ```mermaid\r\n> graph TD\r\n> A-->B\r\n> ```\r\n",
            "- item\n\n  ````math\n  x^2\n  ````",
        ] {
            let (p, n) = parse(s);
            assert_eq!(n.len(), 1);
            assert!(!n[0].source.starts_with('>'));
            assert!(!n[0].source.starts_with("  "));
            let next = patch(s, hash(s), &n[0], "x+1").unwrap();
            assert!(next.contains(&format!("{}x+1", n[0].prefix)));
            assert!(p.contains("$$mdcard0$$"));
        }
    }
    #[test]
    fn links_money_and_tokens_are_protected() {
        let s =
            "[cost](file$x$.md \"$title$\") ![$alt$](a.png) <https://host/$x$> $100 $4$ $mdcard0$";
        let (p, n) = parse(s);
        assert_eq!(
            n.iter().map(|n| n.source.as_str()).collect::<Vec<_>>(),
            ["4", "mdcard0"]
        );
        assert!(p.contains("file$x$.md"));
        assert!(p.contains("\\$100"));
    }
    #[test]
    fn wrapper_changes_cannot_retarget_following_node() {
        let s = "$x$ and $y$";
        let (_, n) = parse(s);
        for replacement in ["", "a$b", "$a$ and $z$", "a\nb"] {
            assert!(
                patch(s, hash(s), &n[0], replacement).is_err(),
                "{replacement:?}"
            );
        }
        assert!(patch(s, hash(s), &n[0], "\\$5+x").is_ok());
    }
    #[test]
    fn preserve_node_mixed_endings() {
        let s = "```math\r\na\r\nb\nc\r\n```\r\n";
        let (_, n) = parse(s);
        let next = patch(s, hash(s), &n[0], "a\nb2\nc").unwrap();
        assert_eq!(next, "```math\r\na\r\nb2\nc\r\n```\r\n");
    }
    #[test]
    fn delimiters_currency_and_code() {
        let s = "$100 and $200. $x$ $ y$ $z $ $4$ \\$no$ ` $code$ ` \\(a+b\\) \\[x^2\\] $$y$$\n```math\n\\ce{H2O}\n```\n```text\n$x$\n```";
        let (_, n) = parse(s);
        assert_eq!(
            n.iter().map(|n| n.source.as_str()).collect::<Vec<_>>(),
            ["x", "4", "a+b", "x^2", "y", "\\ce{H2O}"]
        );
    }
    #[test]
    fn exact_duplicate_unicode_crlf_and_stale_patch() {
        let s = "中文😀 $x$ and $x$\r\n";
        let (p, n) = parse(s);
        assert!(p.contains("$mdcard0$ and $mdcard1$"));
        let next = patch(s, hash(s), &n[1], "\\alpha").unwrap();
        assert_eq!(next, "中文😀 $x$ and $\\alpha$\r\n");
        assert!(patch(&next, hash(s), &n[0], "z").is_err());
    }
    #[test]
    fn unclosed_and_links_stay_source() {
        let s = "[label](file.md) [100] $oops\n```mermaid\ngraph TD\n";
        let (p, n) = parse(s);
        assert!(n.is_empty());
        assert_eq!(p, s.replace("$oops", "\\$oops"));
    }
    #[test]
    fn paragraph_list_table_keep_inline_tokens() {
        let s = "- before $a$ after $b$\n\n| x | y |\n|---|---|\n| $c$ | $d$ |\n";
        let (p, n) = parse(s);
        assert_eq!(n.len(), 4);
        assert!(p.starts_with("- before $mdcard0$ after $mdcard1$"));
    }
}
