// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Native viewport adapter and revisioned Rope / Tree-sitter pipeline.
//!
//! A single-slot mailbox retains every InputEdit even when intermediate snapshots
//! are superseded. The parser and its trees never leave their owning thread.
use crate::{egui, rich_nodes};
use ropey::Rope;
use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
    sync::{Arc, Condvar, Mutex, mpsc},
    time::Instant,
};
use tree_sitter::{InputEdit, Parser, Point, Tree};

pub const THRESHOLD: usize = 64 * 1024;
const OVERSCAN: f32 = 420.;
const MAX_WINDOW_CHARS: usize = 24 * 1024;

/// Fenwick cumulative heights: updates and offset lookup are O(log n).
#[derive(Default, Clone)]
pub struct Heights {
    values: Vec<f32>,
    sums: Vec<f32>,
}
impl Heights {
    pub fn new(values: Vec<f32>) -> Self {
        let mut out = Self {
            sums: vec![0.; values.len() + 1],
            values: vec![0.; values.len()],
        };
        for (i, value) in values.into_iter().enumerate() {
            out.set(i, value);
        }
        out
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn total(&self) -> f32 {
        self.prefix(self.len())
    }
    pub fn value(&self, i: usize) -> f32 {
        self.values.get(i).copied().unwrap_or(0.)
    }
    pub fn set(&mut self, i: usize, value: f32) {
        let delta = value - self.values[i];
        self.values[i] = value;
        let mut j = i + 1;
        while j < self.sums.len() {
            self.sums[j] += delta;
            j += j & j.wrapping_neg();
        }
    }
    pub fn prefix(&self, n: usize) -> f32 {
        let mut j = n.min(self.len());
        let mut sum = 0.;
        while j > 0 {
            sum += self.sums[j];
            j &= j - 1;
        }
        sum
    }
    pub fn at(&self, offset: f32) -> usize {
        if self.len() == 0 {
            return 0;
        }
        let mut index = 0;
        let mut accumulated = 0.;
        let mut bit = 1usize << (usize::BITS - 1 - self.len().leading_zeros());
        while bit > 0 {
            let next = index + bit;
            if next <= self.len() && accumulated + self.sums[next] <= offset {
                index = next;
                accumulated += self.sums[next];
            }
            bit >>= 1;
        }
        index.min(self.len() - 1)
    }
}

#[derive(Clone)]
pub struct CodePage {
    pub source: Arc<str>,
    pub language: String,
    pub bytes: Range<usize>,
    pub document_key: u64,
    pub key: u64,
}
#[derive(Clone)]
pub struct ParsedBlock {
    pub source: Range<usize>,
    pub preview: Range<usize>,
    pub key: u64,
    pub nodes: u64,
    pub code: Option<CodePage>,
    pub mapping: Vec<PreviewSourceSegment>,
}
pub struct Parsed {
    pub generation: u64,
    pub revision: u64,
    pub preview: String,
    pub nodes: Vec<rich_nodes::Node>,
    pub blocks: Vec<ParsedBlock>,
    pub headings: Vec<(usize, String, usize)>,
    pub source_hash: u64,
    pub parse_ms: f64,
    pub transform_ms: f64,
    pub changed: Vec<Range<usize>>,
    pub reused_blocks: usize,
    pub parsed_blocks: usize,
    pub incremental: bool,
    pub local_parse: bool,
}
struct Job {
    generation: u64,
    revision: u64,
    rope: Rope,
    edits: Vec<InputEdit>,
    reset: bool,
}
#[derive(Default)]
struct Mailbox {
    job: Option<Job>,
    result: Option<Parsed>,
    timings: VecDeque<ParseTiming>,
    stop: bool,
}
struct ParseTiming {
    generation: u64,
    revision: u64,
    ms: f64,
    local: bool,
    incremental: bool,
}
struct Worker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    fn new(ctx: egui::Context) -> Self {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let inbox = shared.clone();
        let thread = std::thread::Builder::new()
            .name("mdcard-incremental-parser".into())
            .spawn(move || {
                let mut parser = Parser::new();
                parser
                    .set_language(&tree_sitter_md::LANGUAGE.into())
                    .expect("bundled Markdown grammar");
                let mut tree: Option<Tree> = None;
                let mut generation = u64::MAX;
                let mut previous_rope = Rope::new();
                let mut local_tree: Option<(Range<usize>, Tree)> = None;
                let mut syntax_blocks = Vec::new();
                let mut cache = HashMap::<u64, (String, Vec<rich_nodes::Node>, u64)>::new();
                let mut semantic_cache = HashMap::<
                    u64,
                    (
                        Vec<(usize, String, Option<CodePage>)>,
                        Vec<PreviewSourceSegment>,
                        u64,
                    ),
                >::new();
                let mut clock = 0u64;
                loop {
                    let job = {
                        let (lock, wake) = &*inbox;
                        let mut state = lock.lock().unwrap();
                        while state.job.is_none() && !state.stop {
                            state = wake.wait(state).unwrap();
                        }
                        if state.stop {
                            break;
                        }
                        state.job.take().unwrap()
                    };
                    if job.reset || generation != job.generation {
                        tree = None;
                        local_tree = None;
                        generation = job.generation;
                        cache.clear();
                        semantic_cache.clear();
                    }
                    let incremental = tree.is_some();
                    let started = Instant::now();
                    let local = if tree.is_some() {
                        parse_local(
                            &mut parser,
                            &mut syntax_blocks,
                            &previous_rope,
                            &job,
                            &mut local_tree,
                        )
                    } else {
                        None
                    };
                    let local_parse = local.is_some();
                    if let Some(old) = &mut tree {
                        for edit in &job.edits {
                            old.edit(edit);
                        }
                    }
                    let changed = if let Some(ranges) = local {
                        ranges
                    } else {
                        local_tree = None;
                        let next = parser
                            .parse_with_options(
                                &mut |byte, _| -> &[u8] {
                                    if byte >= job.rope.len_bytes() {
                                        return &[];
                                    }
                                    let (chunk, start, _, _) = job.rope.chunk_at_byte(byte);
                                    &chunk.as_bytes()[byte - start..]
                                },
                                tree.as_ref(),
                                None,
                            )
                            .expect("parser not cancelled");
                        let changed = tree
                            .as_ref()
                            .map(|t| {
                                t.changed_ranges(&next)
                                    .map(|r| r.start_byte..r.end_byte)
                                    .collect()
                            })
                            .unwrap_or_else(|| std::iter::once(0..job.rope.len_bytes()).collect());
                        tree = Some(next);
                        syntax_blocks.clear();
                        collect_blocks(tree.as_ref().unwrap().root_node(), &mut syntax_blocks);
                        changed
                    };
                    let parse_ms = started.elapsed().as_secs_f64() * 1000.;
                    previous_rope = job.rope.clone();
                    let transform = Instant::now();
                    let ranges = syntax_blocks.clone();
                    let definitions = ranges
                        .iter()
                        .filter(|(_, kind)| *kind == "link_reference_definition")
                        .map(|(range, _)| job.rope.byte_slice(range.clone()).to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let definition_parser = pulldown_cmark::Parser::new_ext(
                        &definitions,
                        pulldown_cmark::Options::all(),
                    );
                    let definition_hash = rich_nodes::hash(&definitions);
                    let mut preview = String::new();
                    let mut nodes = Vec::new();
                    let mut blocks = Vec::new();
                    let mut headings = Vec::new();
                    let mut reused_blocks = 0;
                    let mut parsed_blocks = 0;
                    clock += 1;
                    for (range, kind) in ranges {
                        let source = job.rope.byte_slice(range.clone()).to_string();
                        if matches!(kind, "atx_heading" | "setext_heading")
                            && let Some((level, text)) =
                                crate::document::outline(&source).into_iter().next()
                        {
                            headings.push((level, text, range.start));
                        }
                        if source.len() > 8192 && kind != "fenced_code_block" {
                            let key = rich_nodes::hash((&source, kind, definition_hash));
                            let (converted, local_nodes) =
                                if let Some((text, n, last)) = cache.get_mut(&key) {
                                    *last = clock;
                                    reused_blocks += 1;
                                    (text.clone(), n.clone())
                                } else {
                                    parsed_blocks += 1;
                                    let parsed = rich_nodes::parse(&source);
                                    cache.insert(key, (parsed.0.clone(), parsed.1.clone(), clock));
                                    parsed
                                };
                            let base = nodes.len();
                            let (pages, source_map) =
                                if let Some((pages, map, last)) = semantic_cache.get_mut(&key) {
                                    *last = clock;
                                    (pages.clone(), map.clone())
                                } else {
                                    let source_map =
                                        preview_source_map(&source, &converted, &local_nodes);
                                    let converted = resolve_references(
                                        &converted,
                                        &definitions,
                                        definition_parser.reference_definitions(),
                                    );
                                    let pages = semantic_pages(&converted);
                                    semantic_cache
                                        .insert(key, (pages.clone(), source_map.clone(), clock));
                                    (pages, source_map)
                                };
                            let mut pages = pages;
                            for (_, text, _) in &mut pages {
                                for local in (0..local_nodes.len()).rev() {
                                    *text = text.replace(
                                        &format!("mdcard{local}$"),
                                        &format!("mdcard{}$", base + local),
                                    );
                                }
                            }
                            let mappings = page_source_maps(&source, &local_nodes, &pages);
                            let mut previous = range.start;
                            let page_count = pages.len();
                            for (index, ((end, text, code), mut mapping)) in
                                pages.into_iter().zip(mappings).enumerate()
                            {
                                if code.is_some() {
                                    let body = code_body_map(&text);
                                    mapping = body
                                        .into_iter()
                                        .filter_map(|part| {
                                            let source =
                                                map_preview_range(&mapping, part.source.clone())?;
                                            Some(PreviewSourceSegment {
                                                preview: part.preview,
                                                linear: part.linear
                                                    && source.len() == part.source.len(),
                                                source,
                                            })
                                        })
                                        .collect();
                                }
                                for span in &mut mapping {
                                    span.source.start += range.start;
                                    span.source.end += range.start;
                                }
                                let mut mapped = if index + 1 == page_count {
                                    range.end
                                } else {
                                    range.start
                                        + map_preview_boundary(&source_map, end, source.len())
                                };
                                while mapped > range.start
                                    && !source.is_char_boundary(mapped - range.start)
                                {
                                    mapped -= 1;
                                }
                                if mapped <= previous {
                                    continue;
                                }
                                let start = preview.len();
                                preview.push_str(&text);
                                preview.push_str("\n\n");
                                blocks.push(ParsedBlock {
                                    source: previous..mapped,
                                    preview: start..preview.len(),
                                    key: rich_nodes::hash((key, index)),
                                    nodes: text.matches("mdcard").count() as u64,
                                    code,
                                    mapping,
                                });
                                previous = mapped;
                            }
                            if previous < range.end {
                                let start = preview.len();
                                blocks.push(ParsedBlock {
                                    source: previous..range.end,
                                    preview: start..start,
                                    key,
                                    nodes: 0,
                                    code: None,
                                    mapping: Vec::new(),
                                });
                            }
                            for mut node in local_nodes {
                                node.range.start += range.start;
                                node.range.end += range.start;
                                node.body.start += range.start;
                                node.body.end += range.start;
                                nodes.push(node);
                            }
                            continue;
                        }
                        // Bound each viewer even for unbroken prose and enormous code fences.
                        let code_original_map = if kind == "fenced_code_block" {
                            code_body_map(&source)
                        } else {
                            Vec::new()
                        };
                        for (piece_range, piece, code) in
                            preview_parts(range.clone(), &source, kind)
                        {
                            let key = rich_nodes::hash((&piece, kind, definition_hash));
                            let (converted, local_nodes) =
                                if let Some((text, n, last)) = cache.get_mut(&key) {
                                    *last = clock;
                                    reused_blocks += 1;
                                    (text.clone(), n.clone())
                                } else {
                                    parsed_blocks += 1;
                                    let parsed = if code.is_some() {
                                        (piece.clone(), Vec::new())
                                    } else {
                                        rich_nodes::parse(&piece)
                                    };
                                    cache.insert(key, (parsed.0.clone(), parsed.1.clone(), clock));
                                    parsed
                                };
                            let preview_start = preview.len();
                            let base = nodes.len();
                            let mut converted = if code.is_some() {
                                converted
                            } else {
                                resolve_references(
                                    &converted,
                                    &definitions,
                                    definition_parser.reference_definitions(),
                                )
                            };
                            // Tokens are indexed per cached partition; remap to snapshot-wide indices.
                            for index in (0..local_nodes.len()).rev() {
                                converted = converted.replace(
                                    &format!("mdcard{index}$"),
                                    &format!("mdcard{}$", base + index),
                                );
                            }
                            preview.push_str(&converted);
                            preview.push_str("\n\n");
                            let count = local_nodes.len() as u64;
                            let mapping = if let Some(page) = &code {
                                slice_projection(
                                    &code_original_map,
                                    page.bytes.clone(),
                                    range.start,
                                )
                            } else {
                                let pages = [(converted.len(), converted.clone(), None)];
                                let mut mapping = page_source_maps(&piece, &local_nodes, &pages)
                                    .pop()
                                    .unwrap_or_default();
                                for span in &mut mapping {
                                    span.source.start += piece_range.start;
                                    span.source.end += piece_range.start;
                                }
                                mapping
                            };
                            for mut node in local_nodes {
                                node.range.start += piece_range.start;
                                node.range.end += piece_range.start;
                                node.body.start += piece_range.start;
                                node.body.end += piece_range.start;
                                nodes.push(node);
                            }
                            blocks.push(ParsedBlock {
                                source: piece_range,
                                preview: preview_start..preview.len(),
                                key,
                                nodes: count,
                                code,
                                mapping,
                            });
                        }
                    }
                    let semantic_size =
                        |pages: &[(usize, String, Option<CodePage>)],
                         map: &[PreviewSourceSegment]| {
                            let mut code_keys = std::collections::HashSet::new();
                            pages
                                .iter()
                                .map(|(_, text, code)| {
                                    text.len()
                                        + code
                                            .as_ref()
                                            .filter(|code| code_keys.insert(code.document_key))
                                            .map_or(0, |code| code.source.len())
                                })
                                .sum::<usize>()
                                + std::mem::size_of_val(map)
                        };
                    let mut semantic_bytes = semantic_cache
                        .values()
                        .map(|(pages, map, _)| semantic_size(pages, map))
                        .sum::<usize>();
                    if semantic_cache.len() > 512 || semantic_bytes > 32 * 1024 * 1024 {
                        let mut order = semantic_cache
                            .iter()
                            .map(|(key, (_, _, age))| (*age, *key))
                            .collect::<Vec<_>>();
                        order.sort_unstable();
                        for (_, key) in order {
                            if semantic_cache.len() <= 512 && semantic_bytes <= 32 * 1024 * 1024 {
                                break;
                            }
                            if let Some((pages, map, _)) = semantic_cache.remove(&key) {
                                semantic_bytes =
                                    semantic_bytes.saturating_sub(semantic_size(&pages, &map));
                            }
                        }
                    }
                    // Cache only recent partitions, with a byte cap as well as an entry cap.
                    let mut bytes = cache
                        .values()
                        .map(|(s, n, _)| {
                            s.len()
                                + n.iter()
                                    .map(|n| n.source.len() + n.raw.len())
                                    .sum::<usize>()
                        })
                        .sum::<usize>();
                    if cache.len() > 4096 || bytes > 32 * 1024 * 1024 {
                        let mut order = cache
                            .iter()
                            .map(|(k, (_, _, age))| (*age, *k))
                            .collect::<Vec<_>>();
                        order.sort_unstable();
                        for (_, key) in order {
                            if cache.len() <= 4096 && bytes <= 32 * 1024 * 1024 {
                                break;
                            }
                            if let Some((s, n, _)) = cache.remove(&key) {
                                bytes = bytes.saturating_sub(
                                    s.len()
                                        + n.iter()
                                            .map(|n| n.source.len() + n.raw.len())
                                            .sum::<usize>(),
                                );
                            }
                        }
                    }
                    let source_hash = {
                        use std::hash::Hasher;
                        let mut h = std::collections::hash_map::DefaultHasher::new(); // Match rich_nodes::hash(String).
                        for chunk in job.rope.chunks() {
                            h.write(chunk.as_bytes());
                        }
                        h.write_u8(0xff);
                        h.finish()
                    };
                    let result = Parsed {
                        generation: job.generation,
                        revision: job.revision,
                        preview,
                        nodes,
                        blocks,
                        headings,
                        source_hash,
                        parse_ms,
                        transform_ms: transform.elapsed().as_secs_f64() * 1000.,
                        changed,
                        reused_blocks,
                        parsed_blocks,
                        incremental,
                        local_parse,
                    };
                    {
                        let mut mailbox = inbox.0.lock().unwrap();
                        if mailbox.stop {
                            break;
                        }
                        if mailbox.timings.len() >= 2048 {
                            mailbox.timings.pop_front();
                        }
                        mailbox.timings.push_back(ParseTiming {
                            generation: result.generation,
                            revision: result.revision,
                            ms: result.parse_ms,
                            local: result.local_parse,
                            incremental: result.incremental,
                        });
                        mailbox.result = Some(result);
                    }
                    ctx.request_repaint();
                }
            })
            .expect("parser thread");
        Self {
            shared,
            thread: Some(thread),
        }
    }
    fn poll(&self) -> (Option<Parsed>, VecDeque<ParseTiming>) {
        let mut mailbox = self.shared.0.lock().unwrap();
        (mailbox.result.take(), std::mem::take(&mut mailbox.timings))
    }
    fn submit(&self, mut job: Job) {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().unwrap();
        if let Some(mut previous) = state.job.take()
            && previous.generation == job.generation
            && !job.reset
        {
            previous.edits.append(&mut job.edits);
            job.edits = previous.edits;
            job.reset = previous.reset;
        }
        state.job = Some(job);
        wake.notify_one();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        lock.lock().unwrap().stop = true;
        wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Fast lane for edits whose entire grammar context is a single prose/heading
/// block. The local Tree is edited and reused; its new block shape must agree
/// with the master boundary tree. Any delimiter/newline/context change takes
/// the full incremental parser path on the worker instead.
fn parse_local(
    parser: &mut Parser,
    blocks: &mut Vec<(Range<usize>, &'static str)>,
    previous: &Rope,
    job: &Job,
    cached: &mut Option<(Range<usize>, Tree)>,
) -> Option<Vec<Range<usize>>> {
    if job.edits.len() != 1 {
        return None;
    }
    let edit = &job.edits[0];
    if edit.old_end_byte > previous.len_bytes() || edit.new_end_byte > job.rope.len_bytes() {
        return None;
    }
    if blocks.is_empty() {
        return None;
    }
    let allowed = |kind: &str| {
        matches!(
            kind,
            "paragraph" | "atx_heading" | "setext_heading" | "link_reference_definition"
        )
    };
    let center = blocks
        .partition_point(|(r, _)| r.end <= edit.start_byte)
        .min(blocks.len() - 1);
    if !allowed(blocks[center].1) {
        return None;
    }
    let first = if center > 0
        && allowed(blocks[center - 1].1)
        && blocks[center].0.end - blocks[center - 1].0.start < 8192
    {
        center - 1
    } else {
        center
    };
    let affected_last = blocks
        .partition_point(|(r, _)| r.start <= edit.old_end_byte)
        .max(center + 1)
        .min(blocks.len());
    let last = if affected_last < blocks.len()
        && allowed(blocks[affected_last].1)
        && blocks[affected_last].0.end - blocks[first].0.start < 8192
    {
        affected_last + 1
    } else {
        affected_last
    };
    let old_range = blocks[first].0.start..blocks[last - 1].0.end;
    if old_range.len() > 8192
        || edit.start_byte < old_range.start
        || edit.old_end_byte > old_range.end
    {
        return None;
    }
    // Opaque contexts (HTML/fences/list state) are deliberately delegated to
    // the document parser; neighboring prose blocks provide synchronization.
    if blocks[first..last].iter().any(|(_, kind)| !allowed(kind)) {
        return None;
    }
    let old_text = previous.byte_slice(old_range.clone()).to_string();
    let delta = edit.new_end_byte as isize - edit.old_end_byte as isize;
    let new_range = old_range.start..old_range.end.checked_add_signed(delta)?;
    let new_text = job.rope.byte_slice(new_range.clone()).to_string();
    let mut old_tree = if let Some((range, tree)) = cached.take() {
        if range == old_range {
            tree
        } else {
            parser.parse(&old_text, None)?
        }
    } else {
        parser.parse(&old_text, None)?
    };
    let local_start = edit.start_byte - old_range.start;
    let local_old_end = edit.old_end_byte - old_range.start;
    let local_new_end = edit.new_end_byte - old_range.start;
    let old_local = Rope::from_str(&old_text);
    let new_local = Rope::from_str(&new_text);
    old_tree.edit(&InputEdit {
        start_byte: local_start,
        old_end_byte: local_old_end,
        new_end_byte: local_new_end,
        start_position: point(&old_local, local_start),
        old_end_position: point(&old_local, local_old_end),
        new_end_position: point(&new_local, local_new_end),
    });
    let next = parser.parse(&new_text, Some(&old_tree))?;
    let mut shape = Vec::new();
    collect_blocks(next.root_node(), &mut shape);
    if shape.is_empty() {
        return None;
    }
    if first > 0 && (shape.first()?.1 != blocks[first].1 || shape.first()?.0.start != 0) {
        return None;
    }
    if last < blocks.len()
        && (shape.last()?.1 != blocks[last - 1].1 || shape.last()?.0.end != new_text.len())
    {
        return None;
    }
    if shape.iter().any(|(_, kind)| !allowed(kind)) {
        return None;
    }
    for (range, _) in blocks.iter_mut().skip(last) {
        range.start = range.start.checked_add_signed(delta)?;
        range.end = range.end.checked_add_signed(delta)?;
    }
    blocks.splice(
        first..last,
        shape
            .into_iter()
            .map(|(r, k)| (r.start + new_range.start..r.end + new_range.start, k)),
    );
    *cached = Some((new_range.clone(), next));
    Some(vec![new_range])
}

fn collect_blocks(node: tree_sitter::Node<'_>, out: &mut Vec<(Range<usize>, &'static str)>) {
    if matches!(node.kind(), "document" | "section") {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect_blocks(child, out);
        }
    } else if node.end_byte() > node.start_byte() {
        out.push((node.byte_range(), node.kind()));
    }
}
fn resolve_references(
    source: &str,
    definitions: &str,
    refs: &pulldown_cmark::RefDefs<'_>,
) -> String {
    if definitions.is_empty() || !source.contains('[') {
        return source.to_owned();
    }
    let mut needed = std::collections::BTreeSet::new();
    pulldown_cmark::Parser::new_with_broken_link_callback(
        source,
        pulldown_cmark::Options::all(),
        Some(|link: pulldown_cmark::BrokenLink<'_>| {
            refs.get(&link.reference).map(|def| {
                needed.insert((def.span.start, def.span.end));
                (
                    def.dest.clone().into_static(),
                    def.title.clone().unwrap_or_else(|| "".into()).into_static(),
                )
            })
        }),
    )
    .for_each(drop);
    let mut out = source.to_owned();
    for (start, end) in needed {
        out.push_str("\n\n");
        out.push_str(&definitions[start..end]);
    }
    out
}
fn preview_parts(
    range: Range<usize>,
    source: &str,
    kind: &str,
) -> Vec<(Range<usize>, String, Option<CodePage>)> {
    if kind != "fenced_code_block" || source.len() <= 8192 {
        return vec![(range, source.into(), None)];
    }
    let first_end = source.find('\n').map_or(source.len(), |n| n + 1);
    let opening = source[..first_end].trim();
    let language = opening
        .trim_start_matches(['`', '~'])
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned();
    if [
        "mermaid",
        "plantuml",
        "puml",
        "math",
        "latex",
        "flow",
        "flowchart",
    ]
    .contains(&language.as_str())
    {
        return vec![(range, source.into(), None)];
    }
    let fence = opening.chars().next().unwrap_or('`');
    let fence_len = opening.chars().take_while(|c| *c == fence).count();
    let trimmed_end = source.trim_end_matches(['\r', '\n']).len();
    let closing_start = source[..trimmed_end]
        .rfind('\n')
        .map_or(first_end, |n| n + 1);
    let closing = source[closing_start..trimmed_end].trim();
    let closed = closing.chars().all(|c| c == fence) && closing.len() >= fence_len;
    let body_end = if closed { closing_start } else { source.len() };
    let body: Arc<str> = Arc::from(&source[first_end..body_end]);
    let document_key = rich_nodes::hash((&*body, &language));
    let mut out = Vec::new();
    let mut start = 0;
    while start < body.len() {
        let mut end = start;
        for (lines, line) in body[start..].split_inclusive('\n').enumerate() {
            if lines > 0 && (lines >= 64 || end - start + line.len() > 8192) {
                break;
            }
            if line.len() > 8192 && lines == 0 {
                end = start + 8192;
                while !body.is_char_boundary(end) {
                    end -= 1;
                }
                break;
            }
            end += line.len();
            if end - start >= 8192 {
                break;
            }
        }
        let page = CodePage {
            source: body.clone(),
            language: language.clone(),
            bytes: start..end,
            document_key,
            key: rich_nodes::hash((document_key, start, end)),
        };
        let first = if start == 0 {
            range.start
        } else {
            range.start + first_end + start
        };
        let last = if end == body.len() {
            range.end
        } else {
            range.start + first_end + end
        };
        out.push((first..last, body[start..end].to_owned(), Some(page)));
        start = end;
    }
    if out.is_empty() {
        out.push((range, source.to_owned(), None));
    }
    out
}

fn point(rope: &Rope, byte: usize) -> Point {
    let row = rope.byte_to_line(byte);
    Point::new(row, byte - rope.line_to_byte(row))
}

#[derive(Clone)]
pub struct PreviewSourceSegment {
    pub(crate) preview: Range<usize>,
    pub(crate) source: Range<usize>,
    pub(crate) linear: bool,
}
pub(crate) fn preview_source_map(
    source: &str,
    preview: &str,
    nodes: &[rich_nodes::Node],
) -> Vec<PreviewSourceSegment> {
    let mut segments: Vec<PreviewSourceSegment> = Vec::new();
    let (mut s, mut p, mut node) = (0, 0, 0);
    while s < source.len() {
        if let Some(n) = nodes.get(node).filter(|n| n.range.start == s) {
            let token = if n.inline {
                format!("$mdcard{node}$")
            } else {
                format!("$$mdcard{node}$$")
            };
            segments.push(PreviewSourceSegment {
                preview: p..p + token.len(),
                source: n.range.clone(),
                linear: false,
            });
            s = n.range.end;
            p += token.len();
            node += 1;
            continue;
        }
        let ch = source[s..].chars().next().unwrap();
        let len = ch.len_utf8();
        if ch == '$' && preview.as_bytes().get(p) == Some(&b'\\') {
            segments.push(PreviewSourceSegment {
                preview: p..p + 1,
                source: s..s,
                linear: false,
            });
            p += 1;
        }
        if let Some(last) = segments
            .last_mut()
            .filter(|last| last.linear && last.preview.end == p && last.source.end == s)
        {
            last.preview.end += len;
            last.source.end += len;
        } else {
            segments.push(PreviewSourceSegment {
                preview: p..p + len,
                source: s..s + len,
                linear: true,
            });
        }
        s += len;
        p += len;
    }
    segments
}
fn code_body_map(source: &str) -> Vec<PreviewSourceSegment> {
    use pulldown_cmark::{Event, Tag, TagEnd};
    let mut active = false;
    let mut at = 0;
    let mut result = Vec::new();
    for (event, range) in
        pulldown_cmark::Parser::new_ext(source, pulldown_cmark::Options::all()).into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(_)) => active = true,
            Event::End(TagEnd::CodeBlock) => active = false,
            Event::Text(value) if active => {
                if let Some(raw) = source.get(range.clone()) {
                    for (display, original) in crate::reading_geometry::fragment_map(raw, &value) {
                        let preview = at + display.start..at + display.end;
                        let source = range.start + original.start..range.start + original.end;
                        let linear = preview.len() == source.len();
                        result.push(PreviewSourceSegment {
                            preview,
                            source,
                            linear,
                        });
                    }
                }
                at += value.len();
            }
            _ => {}
        }
    }
    result
}
fn slice_projection(
    map: &[PreviewSourceSegment],
    range: Range<usize>,
    base: usize,
) -> Vec<PreviewSourceSegment> {
    map.iter()
        .filter_map(|s| {
            let start = range.start.max(s.preview.start);
            let end = range.end.min(s.preview.end);
            if start >= end {
                return None;
            }
            let source = map_preview_range(map, start..end)?;
            Some(PreviewSourceSegment {
                preview: start - range.start..end - range.start,
                source: base + source.start..base + source.end,
                linear: s.linear,
            })
        })
        .collect()
}
/// Match the complete ordered semantic stream, including virtual page
/// boundaries. This is parser provenance, never a first/nearest text search.
/// Synthetic repeated table headings have no source identity on later pages.
fn page_source_maps(
    source: &str,
    nodes: &[rich_nodes::Node],
    pages: &[(usize, String, Option<CodePage>)],
) -> Vec<Vec<PreviewSourceSegment>> {
    use pulldown_cmark::{Event, Tag, TagEnd};
    struct Run {
        value: String,
        source: Range<usize>,
        linear: bool,
    }
    let mut runs = Vec::new();
    let mut image = 0usize;
    for (event, range) in
        pulldown_cmark::Parser::new_ext(source, pulldown_cmark::Options::all()).into_offset_iter()
    {
        match event {
            Event::Start(Tag::Image { .. }) => image += 1,
            Event::End(TagEnd::Image) => image = image.saturating_sub(1),
            Event::Text(value) | Event::Code(value)
                if image == 0
                    && !nodes
                        .get(nodes.partition_point(|n| n.range.end <= range.start))
                        .is_some_and(|n| {
                            n.range.start <= range.start && range.start < n.range.end
                        }) =>
            {
                if let Some(raw) = source.get(range.clone()) {
                    for (display, original) in crate::reading_geometry::fragment_map(raw, &value) {
                        let text = value[display.clone()].to_owned();
                        let mapped = range.start + original.start..range.start + original.end;
                        let linear = display.len() == mapped.len()
                            && source.get(mapped.clone()) == Some(text.as_str());
                        runs.push(Run {
                            value: text,
                            source: mapped,
                            linear,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    struct Cursor {
        run: usize,
        byte: usize,
    }
    impl Cursor {
        fn peek(&mut self, runs: &[Run]) -> Option<(char, Range<usize>)> {
            while self.run < runs.len() && self.byte >= runs[self.run].value.len() {
                self.run += 1;
                self.byte = 0;
            }
            let r = runs.get(self.run)?;
            let ch = r.value[self.byte..].chars().next()?;
            Some((
                ch,
                if r.linear {
                    r.source.start + self.byte..r.source.start + self.byte + ch.len_utf8()
                } else {
                    r.source.clone()
                },
            ))
        }
        fn advance(&mut self, ch: char) {
            self.byte += ch.len_utf8();
        }
    }
    let mut cursor = Cursor { run: 0, byte: 0 };
    let mut result = Vec::new();
    let mut valid = true;
    for (page_index, (_, page, _)) in pages.iter().enumerate() {
        let mut mapped: Vec<PreviewSourceSegment> = Vec::new();
        let mut image = 0usize;
        let mut head = false;
        for (event, range) in
            pulldown_cmark::Parser::new_ext(page, pulldown_cmark::Options::all()).into_offset_iter()
        {
            match event {
                Event::Start(Tag::Image { .. }) => image += 1,
                Event::End(TagEnd::Image) => image = image.saturating_sub(1),
                Event::Start(Tag::TableHead) => head = true,
                Event::End(TagEnd::TableHead) => head = false,
                Event::Text(value) | Event::Code(value)
                    if image == 0 && !(page_index > 0 && head) =>
                {
                    let Some(raw) = page.get(range.clone()) else {
                        continue;
                    };
                    for (display, original) in crate::reading_geometry::fragment_map(raw, &value) {
                        for (offset, ch) in value[display.clone()].char_indices() {
                            let local = display.start + offset;
                            let preview = if display.len() == original.len() {
                                range.start + original.start + offset
                                    ..range.start + original.start + offset + ch.len_utf8()
                            } else {
                                range.start + original.start..range.start + original.end
                            };
                            let mut expected = cursor.peek(&runs);
                            if !ch.is_whitespace() {
                                while expected
                                    .as_ref()
                                    .is_some_and(|(old, _)| old.is_whitespace())
                                {
                                    let old = expected.unwrap().0;
                                    cursor.advance(old);
                                    expected = cursor.peek(&runs);
                                }
                            }
                            let Some((old, source_range)) = expected else {
                                if !ch.is_whitespace() {
                                    valid = false;
                                }
                                continue;
                            };
                            if old != ch {
                                if !ch.is_whitespace() {
                                    valid = false;
                                }
                                continue;
                            }
                            cursor.advance(old);
                            if !valid {
                                continue;
                            }
                            let linear = preview.len() == source_range.len()
                                && page.get(preview.clone()) == source.get(source_range.clone());
                            if let Some(last) = mapped.last_mut().filter(|s| {
                                s.linear
                                    && linear
                                    && s.preview.end == preview.start
                                    && s.source.end == source_range.start
                            }) {
                                last.preview.end = preview.end;
                                last.source.end = source_range.end;
                            } else if !mapped
                                .last()
                                .is_some_and(|s| s.preview == preview && s.source == source_range)
                            {
                                mapped.push(PreviewSourceSegment {
                                    preview,
                                    source: source_range,
                                    linear,
                                });
                            }
                            let _ = local;
                        }
                    }
                }
                _ => {}
            }
        }
        result.push(mapped);
    }
    if !valid {
        for map in &mut result {
            map.clear();
        }
    }
    result
}
pub(crate) fn map_preview_range(
    segments: &[PreviewSourceSegment],
    range: Range<usize>,
) -> Option<Range<usize>> {
    let mut covered = range.start;
    let mut result: Option<Range<usize>> = None;
    let first = segments.partition_point(|s| s.preview.end <= range.start);
    for s in &segments[first..] {
        if s.preview.start >= range.end {
            break;
        }
        let a = range.start.max(s.preview.start);
        let b = range.end.min(s.preview.end);
        if a >= b {
            continue;
        }
        if a > covered {
            return None;
        }
        let mapped = if s.linear {
            s.source.start + a - s.preview.start..s.source.start + b - s.preview.start
        } else if a == s.preview.start && b == s.preview.end {
            s.source.clone()
        } else {
            return None;
        };
        if let Some(last) = &mut result {
            if last.end != mapped.start {
                return None;
            }
            last.end = mapped.end;
        } else {
            result = Some(mapped);
        }
        covered = b;
    }
    (covered == range.end).then_some(result).flatten()
}
pub(crate) fn map_preview_boundary(
    segments: &[PreviewSourceSegment],
    at: usize,
    total: usize,
) -> usize {
    let index = segments.partition_point(|s| s.preview.end < at);
    segments
        .get(index)
        .map_or(total, |s| {
            if at <= s.preview.start {
                s.source.start
            } else if s.linear {
                s.source.start + at.saturating_sub(s.preview.start).min(s.source.len())
            } else {
                s.source.end
            }
        })
        .min(total)
}

/// Reopen AST context at virtual page boundaries. The source spans remain
/// independent: generated emphasis/list/fence markers never enter Document.
fn semantic_pages(text: &str) -> Vec<(usize, String, Option<CodePage>)> {
    use pulldown_cmark::{Event, Tag, TagEnd};
    let options = pulldown_cmark::Options::all();
    let events = pulldown_cmark::Parser::new_ext(text, options)
        .into_offset_iter()
        .map(|(event, range)| (event.into_static(), range))
        .collect::<Vec<_>>();
    let mut code_sources = HashMap::new();
    let mut table_headers = HashMap::new();
    for (index, (event, _)) in events.iter().enumerate() {
        if matches!(event, Event::Start(Tag::Table(_))) {
            let mut header = Vec::new();
            for (event, _) in events.iter().skip(index + 1) {
                header.push(event.clone());
                if matches!(event, Event::End(TagEnd::TableHead)) {
                    break;
                }
            }
            table_headers.insert(index, header);
        }
    }
    for (index, (event, _)) in events.iter().enumerate() {
        if let Event::Start(Tag::CodeBlock(kind)) = event {
            let mut code = String::new();
            for (event, _) in events.iter().skip(index + 1) {
                match event {
                    Event::Text(value) => code.push_str(value),
                    Event::End(TagEnd::CodeBlock) => break,
                    _ => {}
                }
            }
            let language = match kind {
                pulldown_cmark::CodeBlockKind::Fenced(info) => {
                    info.split_whitespace().next().unwrap_or("").to_owned()
                }
                _ => String::new(),
            };
            code_sources.insert(index, (Arc::<str>::from(code), language));
        }
    }
    let mut stack = Vec::<Tag<'static>>::new();
    let mut buffer = Vec::<Event<'static>>::new();
    let mut pages = Vec::new();
    let mut payload = 0usize;
    let mut boundary = 0usize;
    let mut active_code: Option<(Arc<str>, String, usize, usize)> = None;
    let mut table_header = Vec::new();
    #[allow(clippy::too_many_arguments)] // Explicit streaming state at each AST boundary.
    fn flush(
        buffer: &mut Vec<Event<'static>>,
        stack: &[Tag<'static>],
        pages: &mut Vec<(usize, String, Option<CodePage>)>,
        payload: &mut usize,
        boundary: usize,
        code: &mut Option<(Arc<str>, String, usize, usize)>,
        options: pulldown_cmark::Options,
        table_header: &[Event<'static>],
    ) {
        if *payload == 0 {
            return;
        }
        let mut balanced = std::mem::take(buffer);
        for tag in stack.iter().rev() {
            balanced.push(Event::End(tag.to_end()));
        }
        // CommonMark emphasis delimiters cannot open/close against literal
        // whitespace. Character references preserve those exact characters
        // while keeping synthetic page boundary delimiters well formed.
        let mut serializable = Vec::new();
        let mut in_code = false;
        for event in balanced {
            match event {
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code = true;
                    serializable.push(event)
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code = false;
                    serializable.push(event)
                }
                Event::Text(value) if !in_code => {
                    let leading = value.len() - value.trim_start_matches(char::is_whitespace).len();
                    let trailing = value
                        .trim_end_matches(char::is_whitespace)
                        .len()
                        .max(leading);
                    for ch in value[..leading].chars() {
                        serializable.push(Event::InlineHtml(format!("&#{};", ch as u32).into()));
                    }
                    if trailing > leading {
                        serializable.push(Event::Text(value[leading..trailing].to_owned().into()));
                    }
                    for ch in value[trailing..].chars() {
                        serializable.push(Event::InlineHtml(format!("&#{};", ch as u32).into()));
                    }
                }
                _ => serializable.push(event),
            }
        }
        let mut rendered = String::new();
        pulldown_cmark_to_cmark::cmark(serializable.iter(), &mut rendered, options)
            .expect("balanced Markdown event stream");
        let code_page = code.as_mut().and_then(|(source, language, start, end)| {
            if *end <= *start {
                return None;
            }
            let document_key = rich_nodes::hash((&**source, &*language));
            let page = CodePage {
                source: source.clone(),
                language: language.clone(),
                bytes: *start..*end,
                document_key,
                key: rich_nodes::hash((document_key, *start, *end)),
            };
            *start = *end;
            Some(page)
        });
        pages.push((boundary, rendered, code_page));
        buffer.clear();
        for tag in stack.iter().cloned() {
            let table = matches!(tag, Tag::Table(_));
            buffer.push(Event::Start(tag));
            if table && !stack.iter().any(|t| matches!(t, Tag::TableHead)) {
                buffer.extend_from_slice(table_header);
            }
        }
        *payload = 0;
    }
    for (index, (event, range)) in events.iter().enumerate() {
        match event {
            Event::Start(tag) => {
                if matches!(tag, Tag::Table(_)) {
                    table_header = table_headers[&index].clone();
                }
                if matches!(tag, Tag::CodeBlock(_)) {
                    flush(
                        &mut buffer,
                        &stack,
                        &mut pages,
                        &mut payload,
                        boundary,
                        &mut active_code,
                        options,
                        &table_header,
                    );
                    let (source, language) = code_sources[&index].clone();
                    active_code = Some((source, language, 0, 0));
                }
                stack.push(tag.clone());
                buffer.push(event.clone());
            }
            Event::End(end) => {
                buffer.push(event.clone());
                stack.pop();
                boundary = boundary.max(range.end);
                if *end == TagEnd::Item
                    && let Some(Tag::List(Some(number))) = stack
                        .iter_mut()
                        .rev()
                        .find(|tag| matches!(tag, Tag::List(_)))
                {
                    *number += 1;
                }
                if matches!(end, TagEnd::CodeBlock) {
                    flush(
                        &mut buffer,
                        &stack,
                        &mut pages,
                        &mut payload,
                        boundary,
                        &mut active_code,
                        options,
                        &table_header,
                    );
                    active_code = None;
                } else if payload >= 4096
                    && matches!(end, TagEnd::Paragraph | TagEnd::Item | TagEnd::TableRow)
                {
                    flush(
                        &mut buffer,
                        &stack,
                        &mut pages,
                        &mut payload,
                        boundary,
                        &mut active_code,
                        options,
                        &table_header,
                    );
                }
                if *end == TagEnd::Table {
                    table_header.clear();
                }
            }
            Event::Text(value) => {
                let table_row = stack.iter().any(|tag| matches!(tag, Tag::TableRow));
                let mut offset = 0;
                while offset < value.len() {
                    let mut end = if table_row {
                        value.len()
                    } else {
                        (offset + 4096usize.saturating_sub(payload).max(1)).min(value.len())
                    };
                    if end < value.len() {
                        end = crate::reading_precision::grapheme_range(value, end).start;
                    }
                    if end <= offset {
                        end = crate::reading_precision::grapheme_range(value, offset).end;
                    }
                    buffer.push(Event::Text(value[offset..end].to_owned().into()));
                    payload += end - offset;
                    if let Some((_, _, _, code_end)) = &mut active_code {
                        *code_end += end - offset;
                    }
                    boundary = boundary.max((range.start + end).min(range.end));
                    offset = end;
                    if payload >= 4096 && !table_row {
                        flush(
                            &mut buffer,
                            &stack,
                            &mut pages,
                            &mut payload,
                            boundary,
                            &mut active_code,
                            options,
                            &table_header,
                        );
                    }
                }
            }
            _ => {
                buffer.push(event.clone());
                payload += match event {
                    Event::Code(v)
                    | Event::Html(v)
                    | Event::InlineHtml(v)
                    | Event::InlineMath(v)
                    | Event::DisplayMath(v) => v.len(),
                    _ => 1,
                };
                boundary = boundary.max(range.end);
                if payload >= 4096 {
                    flush(
                        &mut buffer,
                        &stack,
                        &mut pages,
                        &mut payload,
                        boundary,
                        &mut active_code,
                        options,
                        &table_header,
                    );
                }
            }
        }
    }
    flush(
        &mut buffer,
        &stack,
        &mut pages,
        &mut payload,
        text.len(),
        &mut active_code,
        options,
        &table_header,
    );
    pages
}

#[derive(Default)]
pub struct PaintMetrics {
    pub input_completions: VecDeque<serde_json::Value>,
    pub qpc_frequency: u64,
    pub intervals_ms: VecDeque<f64>,
    pub input_to_complete_ms: VecDeque<f64>,
    pub frame_to_complete_ms: VecDeque<f64>,
    pub last: Option<Instant>,
    pub frames: u64,
    pub first_complete_ms: Option<f64>,
    pub gpu_finish: bool,
}
#[derive(Default)]
pub struct LargeDocument {
    pub repaint_causes: Vec<String>,
    pub paint: Arc<Mutex<PaintMetrics>>,
    pub rope: Rope,
    pub revision: u64,
    generation: Option<u64>,
    document_revision: u64,
    worker: Option<Worker>,
    pub parsed: Option<Parsed>,
    pub installed_revision: Option<u64>,
    pub heights: Heights,
    pub line_heights: Heights,
    measured_line_heights: HashMap<usize, f32>,
    line_height_stamp: u64,
    width: f32,
    font: f32,
    wrap: bool,
    pub visible_source: Range<usize>,
    pub laid_out_lines: usize,
    pub preview_range: Range<usize>,
    pub preview_mounted: usize,
    pub source_anchor: usize,
    pub preview_anchor: usize,
    pub jump: Option<usize>,
    pub preview_jump: Option<usize>,
    pub last_source_offset: f32,
    pub last_source_horizontal_offset: f32,
    pub last_preview_offset: f32,
    pub sync_from_source: bool,
    pub sync_from_preview: bool,
    pub input_ms: VecDeque<f64>,
    pub frame_cpu_ms: VecDeque<f64>,
    pub frame_intervals_ms: VecDeque<f64>,
    pub scroll_intervals_ms: VecDeque<f64>,
    pub parse_samples_ms: VecDeque<f64>,
    pub parse_records: VecDeque<serde_json::Value>,
    pub parse_revision: u64,
    pub active: bool,
    pub frame_count: u64,
    pub first_interactive_ms: Option<f64>,
    pub first_preview_ms: Option<f64>,
    opened: Option<Instant>,
    pub frame_started: Option<Instant>,
    last_frame: Option<Instant>,
    pub input_frame: bool,
    pub scroll_frame: bool,
    pub source_rect: Option<egui::Rect>,
    pub preview_rect: Option<egui::Rect>,
    pub source_scrollbar: Option<egui::Rect>,
    pub preview_scrollbar: Option<egui::Rect>,
    pub toc_scrollbar: Option<egui::Rect>,
    pub toc_controls: Vec<serde_json::Value>,
    pub selection: [usize; 2],
    pending_drag_anchor: Option<usize>,
    pending_external_selection: bool,
    last_window: Range<usize>,
    kernel_state: egui::text_edit::TextEditState,
    reporter: Option<mpsc::SyncSender<Vec<u8>>>,
    report_path: Option<std::path::PathBuf>,
    load_started: Option<Instant>,
    pub preview_height_stamp: u64,
    caret_column: Option<usize>,
    ime_composing: bool,
    last_galley: Option<Arc<egui::Galley>>,
    caret_x: Option<f32>,
    pub selection_trace: VecDeque<serde_json::Value>,
}
impl LargeDocument {
    pub fn opening(&mut self) {
        self.load_started = Some(Instant::now());
    }
    pub fn ensure(
        &mut self,
        text: &str,
        generation: u64,
        document_revision: u64,
        ctx: &egui::Context,
    ) {
        self.active = text.len() >= THRESHOLD;
        if !self.active {
            return;
        }
        if self.generation != Some(generation) {
            self.rope = Rope::from_str(text);
            self.generation = Some(generation);
            self.revision = 0;
            self.document_revision = document_revision;
            self.parsed = None;
            self.installed_revision = None;
            self.opened = self.load_started.take().or_else(|| Some(Instant::now()));
            self.first_interactive_ms = None;
            self.first_preview_ms = None;
            self.line_heights = Heights::default();
            self.heights = Heights::default();
            self.width = 0.;
            self.jump = None;
            self.last_window = 0..0;
            if self.worker.is_none() {
                self.worker = Some(Worker::new(ctx.clone()));
            }
            self.submit(Vec::new(), true);
        } else if self.document_revision != document_revision {
            // External structural / replace / history transactions are explicit, rare full snapshots.
            let before = self.rope.to_string();
            let (a, b, c) = diff(&before, text);
            self.replace(a..b, &text[a..c]);
            self.document_revision = document_revision;
        }
        if let Some(worker) = &self.worker {
            let (result, timings) = worker.poll();
            for timing in timings {
                if timing.generation == generation && timing.incremental {
                    bounded_push(&mut self.parse_samples_ms, timing.ms);
                    if self.parse_records.len() >= 2048 {
                        self.parse_records.pop_front();
                    }
                    self.parse_records.push_back(serde_json::json!({"revision":timing.revision,"ms":timing.ms,"local":timing.local,"current_at_poll":timing.revision==self.revision}));
                }
            }
            if let Some(parsed) = result
                && parsed.generation == generation
                && parsed.revision == self.revision
            {
                self.parse_revision = parsed.revision;
                self.parsed = Some(parsed);
                if self.first_preview_ms.is_none() {
                    self.first_preview_ms =
                        self.opened.map(|at| at.elapsed().as_secs_f64() * 1000.);
                }
            }
        }
    }
    fn submit(&self, edits: Vec<InputEdit>, reset: bool) {
        if let Some(worker) = &self.worker {
            worker.submit(Job {
                generation: self.generation.unwrap(),
                revision: self.revision,
                rope: self.rope.clone(),
                edits,
                reset,
            });
        }
    }
    pub fn replace(&mut self, bytes: Range<usize>, insert: &str) {
        let start_position = point(&self.rope, bytes.start);
        let old_end_position = point(&self.rope, bytes.end);
        let a = self.rope.byte_to_char(bytes.start);
        let b = self.rope.byte_to_char(bytes.end);
        let old_lines = self.rope.len_lines();
        self.rope.remove(a..b);
        self.rope.insert(a, insert);
        self.revision += 1;
        let new_end_byte = bytes.start + insert.len();
        let new_end_position = point(&self.rope, new_end_byte);
        self.submit(
            vec![InputEdit {
                start_byte: bytes.start,
                old_end_byte: bytes.end,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            }],
            false,
        );
        if old_lines != self.rope.len_lines() {
            self.line_heights = Heights::default();
        } else if self.line_heights.len() == self.rope.len_lines() {
            let start = start_position.row;
            let end = new_end_position.row.min(self.line_heights.len() - 1);
            for i in start..=end {
                self.line_heights.set(i, self.estimate_line(i));
            }
        }
    }
    fn estimate_line(&self, line: usize) -> f32 {
        let chars = self.rope.line(line).len_chars().saturating_sub(1);
        let rows = if self.wrap {
            (chars as f32 * self.font * 0.61 / self.width.max(80.))
                .ceil()
                .max(1.)
        } else {
            1.
        };
        rows * (self.font * 1.3).ceil()
    }
    pub fn source_y(&self, byte: usize) -> f32 {
        let byte = byte.min(self.rope.len_bytes());
        let line = self.rope.byte_to_line(byte);
        let start = self.rope.line_to_byte(line);
        let fraction = (byte - start) as f32 / self.rope.line(line).len_bytes().max(1) as f32;
        self.line_heights.prefix(line) + fraction * self.line_heights.value(line)
    }
    pub fn preview_y(&self, byte: usize) -> f32 {
        let Some(parsed) = &self.parsed else {
            return 0.;
        };
        let index = parsed
            .blocks
            .partition_point(|b| b.source.end <= byte)
            .min(parsed.blocks.len().saturating_sub(1));
        let Some(block) = parsed.blocks.get(index) else {
            return 0.;
        };
        let fraction =
            byte.saturating_sub(block.source.start) as f32 / block.source.len().max(1) as f32;
        self.heights.prefix(index) + fraction.clamp(0., 1.) * self.heights.value(index)
    }
    pub fn preview_byte(&self, y: f32) -> usize {
        let Some(parsed) = &self.parsed else {
            return 0;
        };
        let i = self.heights.at(y);
        parsed.blocks.get(i).map_or(0, |b| {
            b.source.start
                + ((y - self.heights.prefix(i)) / self.heights.value(i).max(1.)
                    * b.source.len() as f32)
                    .clamp(0., b.source.len() as f32) as usize
        })
    }
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        if std::env::var_os("MDCARD_PERFORMANCE_REPORT").is_some() {
            self.repaint_causes = ctx
                .repaint_causes()
                .iter()
                .map(|c| format!("{c:?}"))
                .collect();
        }
        let now = Instant::now();
        self.frame_started = Some(now);
        self.frame_count += 1;
        let input = ctx.input(|i| {
            (
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Ime(_)
                            | egui::Event::Key { pressed: true, .. }
                    )
                }),
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::MouseWheel { .. })),
            )
        });
        self.input_frame = input.0;
        self.scroll_frame = input.1;
        if let Some(last) = self.last_frame {
            let ms = now.duration_since(last).as_secs_f64() * 1000.;
            bounded_push(&mut self.frame_intervals_ms, ms);
            if self.scroll_frame {
                bounded_push(&mut self.scroll_intervals_ms, ms);
            }
        }
        self.last_frame = Some(now);
    }
    pub fn finish_frame(&mut self) {
        if let Some(start) = self.frame_started {
            let ms = start.elapsed().as_secs_f64() * 1000.;
            bounded_push(&mut self.frame_cpu_ms, ms);
            if self.input_frame {
                bounded_push(&mut self.input_ms, ms);
            }
            if self.first_interactive_ms.is_none() && self.laid_out_lines > 0 {
                self.first_interactive_ms = self.opened.map(|t| t.elapsed().as_secs_f64() * 1000.);
            }
        }
    }
    pub fn completion_start(&self) -> Option<Instant> {
        self.opened
    }
    pub fn report(&mut self, rich_cache: serde_json::Value, renderer: &str, generation: u64) {
        if self.reporter.is_none() {
            let Some(path) =
                std::env::var_os("MDCARD_PERFORMANCE_REPORT").map(std::path::PathBuf::from)
            else {
                return;
            };
            let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
            let output = path.clone();
            std::thread::spawn(move || {
                while let Ok(bytes) = rx.recv() {
                    let _ = crate::document::atomic_write(&output, &bytes);
                }
            });
            self.reporter = Some(tx);
            self.report_path = Some(path);
        }
        let parsed = self.parsed.as_ref();
        let mut report = serde_json::json!({"schema":1,"platform":std::env::consts::OS,"renderer":renderer,"active":self.active,"generation":generation,"revision":self.revision,"parse_revision":self.parse_revision,"bytes":self.rope.len_bytes(),"characters":self.rope.len_chars(),"lines":self.rope.len_lines(),"source_window":[self.visible_source.start,self.visible_source.end],"laid_out_lines":self.laid_out_lines,"preview_blocks":parsed.map_or(0,|p|p.blocks.len()),"preview_range":[self.preview_range.start,self.preview_range.end],"preview_mounted":self.preview_mounted,"source_rect":self.source_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"preview_rect":self.preview_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"source_horizontal_offset":self.last_source_horizontal_offset,"source_scrollbar":self.source_scrollbar.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"preview_scrollbar":self.preview_scrollbar.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"selection":self.selection,"selection_trace":self.selection_trace,"source_offset":self.last_source_offset,"preview_offset":self.last_preview_offset,"source_anchor":self.source_anchor,"preview_anchor":self.preview_anchor,"source_height":self.line_heights.total(),"preview_height":self.heights.total(),});
        let extra = serde_json::json!({"toc_scrollbar":self.toc_scrollbar.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"toc_controls":self.toc_controls,"disk_open_to_interactive_update_ms":self.first_interactive_ms,"disk_open_to_preview_update_ms":self.first_preview_ms,"input_event_to_update_finish_ms":self.input_ms,"update_cpu_ms":self.frame_cpu_ms,"frame_intervals_ms":self.frame_intervals_ms,"wheel_frame_intervals_ms":self.scroll_intervals_ms,"incremental_parse_ms":self.parse_samples_ms,"parse_records":self.parse_records,"latest_parse_ms":parsed.map(|p|p.parse_ms),"latest_transform_ms":parsed.map(|p|p.transform_ms),"changed_ranges":parsed.map(|p|p.changed.iter().map(|r|[r.start,r.end]).collect::<Vec<_>>()),"reused_blocks":parsed.map_or(0,|p|p.reused_blocks),"parsed_blocks":parsed.map_or(0,|p|p.parsed_blocks),"headings":parsed.map(|p|p.headings.iter().take(50).map(|(level,title,byte)|serde_json::json!({"level":level,"title":title,"byte":byte})).collect::<Vec<_>>()),"frame":self.frame_count,"cache":rich_cache,"note":"Update CPU and update completion are not hardware presentation latency; frame intervals include idle time. No slow samples are filtered."});
        report
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let paint = self.paint.lock().unwrap();
        report["repaint_causes"] = serde_json::json!(self.repaint_causes);
        report["paint"] = serde_json::json!({"frames":paint.frames,"gpu_finish":paint.gpu_finish,"disk_open_to_first_complete_ms":paint.first_complete_ms,"intervals_ms":paint.intervals_ms,"input_to_complete_ms":paint.input_to_complete_ms,"frame_to_complete_ms":paint.frame_to_complete_ms,"input_completions":paint.input_completions,"qpc_frequency":paint.qpc_frequency,"note":"OpenGL tail callback. input_to_complete_ms starts at update, excluding the OS queue. Windows input_completions QPC enables independent injection-to-GPU timing. Neither is swapchain presentation or scanout."});
        drop(paint);
        if let Ok(bytes) = serde_json::to_vec(&report) {
            let _ = self.reporter.as_ref().unwrap().try_send(bytes);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show_source(
        &mut self,
        ui: &mut egui::Ui,
        document: &mut crate::Document,
        id: egui::Id,
        font: f32,
        wrap: bool,
        focus: bool,
        reading: &mut crate::reading_state::ReadingState,
        reading_key: &str,
        open_generation: u64,
    ) -> (egui::Response, bool) {
        reading.layer(reading_key, ui.layer_id());
        self.pending_external_selection = false;
        let width = ui.available_width();
        self.laid_out_lines = 0;
        if (width - self.width).abs() > 1.
            || font != self.font
            || wrap != self.wrap
            || self.line_heights.len() != self.rope.len_lines()
        {
            self.width = width;
            self.font = font;
            self.wrap = wrap;
            self.measured_line_heights.clear();
            self.line_height_stamp = self.line_height_stamp.wrapping_add(1);
            self.line_heights = Heights::new(
                (0..self.rope.len_lines())
                    .map(|i| self.estimate_line(i))
                    .collect(),
            );
        }
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        let reading_layout = format!("large-source:{width:.1}:{font}:{wrap}");
        if let Some(heights) = reading.take_calibration(document, reading_key, &reading_layout) {
            for (line, height) in heights {
                let line = line as usize;
                if line < self.line_heights.len() && height.is_finite() && height > 0. {
                    self.line_heights.set(line, height);
                    self.measured_line_heights.insert(line, height);
                }
            }
        }
        if let Some(selection) = reading.selection(document, reading_key) {
            state
                .cursor
                .set_char_range(Some(crate::reading_state::cursor(selection)));
        }
        let mut global = state.cursor.char_range().unwrap_or_default();
        let entry_selection = [global.primary.index, global.secondary.index];
        let entry_events = ui.input(|i| {
            i.events
                .iter()
                .map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed,
                        modifiers,
                        ..
                    } => format!("{key:?}:{pressed}:{modifiers:?}"),
                    egui::Event::Text(t) => format!("text:{}", t.len()),
                    egui::Event::Paste(t) => format!("paste:{}", t.len()),
                    egui::Event::PointerMoved(_) => "pointer".into(),
                    _ => "other".into(),
                })
                .collect::<Vec<_>>()
        });
        let focused = focus || ui.memory(|m| m.has_focus(id));
        let mut changed = false;
        ui.input(|i| {
            for event in &i.events {
                if let egui::Event::Ime(ime) = event {
                    match ime {
                        egui::ImeEvent::Preedit(_) => self.ime_composing = true,
                        egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled => {
                            self.ime_composing = false
                        }
                        _ => {}
                    }
                }
            }
        });
        // Full-document commands are handled before the bounded native TextEdit.
        if focused {
            let mut edits = Vec::new();
            let mut copied = None;
            ui.input_mut(|input| {
                input.events.retain(|event| match event {
                    egui::Event::Copy => {
                        let r = global.as_sorted_char_range();
                        copied = Some(self.rope.slice(r).to_string());
                        false
                    }
                    egui::Event::Cut => {
                        let r = global.as_sorted_char_range();
                        copied = Some(self.rope.slice(r.clone()).to_string());
                        edits.push((r, String::new()));
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::A,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.command => {
                        global = cursor_range(
                            egui::text::CCursor::new(self.rope.len_chars()),
                            egui::text::CCursor::new(0),
                        );
                        self.pending_external_selection = true;
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::Home | egui::Key::End,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.command || modifiers.ctrl => {
                        let end = matches!(
                            event,
                            egui::Event::Key {
                                key: egui::Key::End,
                                ..
                            }
                        );
                        let at = if end { self.rope.len_chars() } else { 0 };
                        let anchor = if modifiers.shift {
                            global.secondary.index
                        } else {
                            at
                        };
                        global = cursor_range(
                            egui::text::CCursor::new(at),
                            egui::text::CCursor::new(anchor),
                        );
                        self.jump = Some(self.rope.char_to_byte(at));
                        self.pending_external_selection = true;
                        false
                    }
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if matches!(
                        key,
                        egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::PageUp
                            | egui::Key::PageDown
                            | egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::Home
                            | egui::Key::End
                    ) && !modifiers.command
                        && !modifiers.alt
                        && !self.ime_composing =>
                    {
                        let old = global.primary.index.min(self.rope.len_chars());
                        let line = self.rope.char_to_line(old);
                        let start = self.rope.line_to_char(line);
                        let column = old - start;
                        let at = match key {
                            egui::Key::ArrowLeft => {
                                self.caret_column = None;
                                if !modifiers.shift && !global.is_empty() {
                                    global.as_sorted_char_range().start
                                } else {
                                    old.saturating_sub(1)
                                }
                            }
                            egui::Key::ArrowRight => {
                                self.caret_column = None;
                                if !modifiers.shift && !global.is_empty() {
                                    global.as_sorted_char_range().end
                                } else {
                                    (old + 1).min(self.rope.len_chars())
                                }
                            }
                            egui::Key::Home => {
                                self.caret_column = None;
                                start
                            }
                            egui::Key::End => {
                                self.caret_column = None;
                                start
                                    + self.rope.line(line).len_chars().saturating_sub(usize::from(
                                        line + 1 < self.rope.len_lines(),
                                    ))
                            }
                            _ => {
                                let col = *self.caret_column.get_or_insert(column);
                                let rows = if matches!(key, egui::Key::PageUp | egui::Key::PageDown)
                                {
                                    (ui_clip_height(ui) / (font * 1.3)).max(1.) as usize
                                } else {
                                    1
                                };
                                if let Some(galley) = self.last_galley.as_ref().filter(|_| {
                                    old >= self.last_window.start && old <= self.last_window.end
                                }) {
                                    let mut cursor =
                                        egui::text::CCursor::new(old - self.last_window.start);
                                    for _ in 0..rows {
                                        let (next, x) = if matches!(
                                            key,
                                            egui::Key::ArrowUp | egui::Key::PageUp
                                        ) {
                                            galley.cursor_up_one_row(&cursor, self.caret_x)
                                        } else {
                                            galley.cursor_down_one_row(&cursor, self.caret_x)
                                        };
                                        cursor = next;
                                        self.caret_x = x;
                                    }
                                    self.last_window.start + cursor.index
                                } else {
                                    let target =
                                        if matches!(key, egui::Key::ArrowUp | egui::Key::PageUp) {
                                            line.saturating_sub(rows)
                                        } else {
                                            (line + rows).min(self.rope.len_lines() - 1)
                                        };
                                    let n = self.rope.line(target).len_chars().saturating_sub(
                                        usize::from(target + 1 < self.rope.len_lines()),
                                    );
                                    self.rope.line_to_char(target) + col.min(n)
                                }
                            }
                        };
                        let anchor = if modifiers.shift {
                            global.secondary.index
                        } else {
                            at
                        };
                        global = cursor_range(
                            egui::text::CCursor::new(at),
                            egui::text::CCursor::new(anchor),
                        );
                        let byte = self.rope.char_to_byte(at);
                        let y = self.source_y(byte);
                        let height = self.source_rect.map_or(500., |r| r.height());
                        if y < self.last_source_offset
                            || y > self.last_source_offset + height
                            || at <= self.last_window.start
                            || at >= self.last_window.end
                        {
                            self.jump = Some(byte);
                        }
                        self.pending_external_selection = true;
                        false
                    }
                    egui::Event::Text(text) | egui::Event::Paste(text)
                        if text.len() > 8192
                            || global.as_sorted_char_range().start < self.last_window.start
                            || global.as_sorted_char_range().end > self.last_window.end =>
                    {
                        edits.push((global.as_sorted_char_range(), text.clone()));
                        false
                    }
                    egui::Event::Key {
                        key: egui::Key::Backspace | egui::Key::Delete,
                        pressed: true,
                        ..
                    } if !global.is_empty()
                        && (global.as_sorted_char_range().start < self.last_window.start
                            || global.as_sorted_char_range().end > self.last_window.end) =>
                    {
                        edits.push((global.as_sorted_char_range(), String::new()));
                        false
                    }
                    _ => true,
                });
            });
            if let Some(text) = copied {
                ui.ctx().copy_text(text);
            }
            for (range, text) in edits {
                let before = global;
                let mut undoer = state.undoer();
                undoer.add_undo(&(before, document.text.clone()));
                let byte_range =
                    self.rope.char_to_byte(range.start)..self.rope.char_to_byte(range.end);
                self.replace(byte_range.clone(), &text);
                document.text.replace_range(byte_range, &text);
                document.mark_text_changed();
                self.document_revision = document.revision();
                let at = range.start + text.chars().count();
                global = egui::text::CCursorRange::one(egui::text::CCursor::new(at));
                self.jump = Some(self.rope.char_to_byte(at));
                undoer.feed_state(ui.input(|i| i.time), &(global, document.text.clone()));
                state.set_undoer(undoer);
                changed = true;
            }
        }
        if let Some(byte) = self.jump {
            self.source_anchor = byte;
            self.preview_jump = Some(byte);
            self.sync_from_source = true;
        }
        let mut area = if wrap {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        }
        .id_salt(("source-scroll", document.id, id))
        .auto_shrink([false; 2]);
        let reading_restore = reading.plan(
            document,
            reading_key,
            open_generation,
            &reading_layout,
            ui.ctx(),
            ui.available_rect_before_wrap(),
            self.jump.is_some() || self.sync_from_preview,
        );
        if self.last_window.is_empty()
            && reading.awaiting_initial_source_index(document, reading_key)
        {
            // Do not measure an unrelated temporary top while history waits
            // for the new buffer's index. Its actual prefix heights would
            // otherwise alter identical-layout raw offsets on reopening.
            state.cursor.set_char_range(Some(global));
            state.store(ui.ctx(), id);
            self.selection = [global.primary.index, global.secondary.index];
            let viewport = ui.available_rect_before_wrap();
            self.source_rect = Some(viewport);
            reading.viewport(reading_key, viewport);
            reading.await_source_layout(reading_key);
            let response = ui.interact(viewport, id, egui::Sense::click());
            ui.spinner();
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(20));
            return (response, changed);
        }
        reading.layout_readiness(reading_key, false, Vec::new());
        let mut reading_target = reading_restore.as_ref().map(|restore| {
            if restore.exact {
                restore.position.offset
            } else {
                let byte = crate::reading_state::byte_floor(
                    &document.text,
                    restore
                        .position
                        .text_anchor
                        .as_ref()
                        .map_or(restore.position.anchor.byte, |t| t.source.source_byte),
                );
                let line = self.rope.byte_to_line(byte);
                [
                    restore.position.offset[0],
                    (self.source_y(byte)
                        + restore.position.anchor.fraction * self.line_heights.value(line)
                        - restore.position.anchor.viewport_y)
                        .max(0.),
                ]
            }
        });
        reading_target = reading_target.map(|target| reading.scroll_target(reading_key, target));
        if let Some(target) = reading_target {
            self.sync_from_preview = false;
            self.sync_from_source = false;
            area = area
                .horizontal_scroll_offset(target[0])
                .vertical_scroll_offset(target[1]);
        }
        let driven_by_preview = self.sync_from_preview;
        if self.jump.is_some() || self.sync_from_preview {
            let byte = self.jump.unwrap_or(self.preview_anchor);
            area = area.vertical_scroll_offset(self.source_y(byte));
            if !wrap && self.jump.is_some() {
                let line = self.rope.byte_to_line(byte);
                let column = self.rope.byte_to_char(byte) - self.rope.line_to_char(line);
                area = area
                    .horizontal_scroll_offset((column as f32 * font * 0.61 - width * 0.7).max(0.));
            }
            self.sync_from_preview = false;
        }
        if self.pending_drag_anchor.is_some()
            && ui.input(|i| i.pointer.primary_down())
            && let (Some(rect), Some(pointer)) =
                (self.source_rect, ui.input(|i| i.pointer.interact_pos()))
        {
            let delta = if pointer.y > rect.bottom() - 18. {
                (pointer.y - rect.bottom() + 18.).clamp(0., 80.)
            } else if pointer.y < rect.top() + 18. {
                (pointer.y - rect.top() - 18.).clamp(-80., 0.)
            } else {
                0.
            };
            if delta != 0. {
                area = area.vertical_scroll_offset((self.last_source_offset + delta).max(0.));
                ui.ctx().request_repaint();
            }
        }
        let mut response = None;
        let source_layout_metrics = reading.layout_metrics.clone();
        let mut source_galleys = Vec::new();
        // A restored giant logical line keeps its semantic window around the
        // same glyph after pending ends. Otherwise the estimated row window can
        // change at every compensation and produce a feedback loop.
        let precise_byte = reading.precise_byte(reading_key);
        let mut out=area.show_viewport(ui,|ui,viewport|{
            let total=self.line_heights.total();ui.set_min_height(total.max(viewport.height()));ui.set_min_width(width);
            let mut first=self.line_heights.at(viewport.min.y.max(0.));let mut last=self.line_heights.at(viewport.max.y+OVERSCAN)+1;
            if let Some(byte)=precise_byte {
                let pinned_line=self.rope.byte_to_line(byte.min(self.rope.len_bytes()));
                if pinned_line<first || pinned_line>=last {
                    first=pinned_line;
                    last=self.line_heights.at(self.line_heights.prefix(first)+viewport.height()+OVERSCAN)+1;
                }
            }
            let start=self.rope.line_to_char(first);let mut end=self.rope.line_to_char(last.min(self.rope.len_lines()-1));if last>=self.rope.len_lines(){end=self.rope.len_chars();}
            // A giant line is a separate bounded window, with its own row estimate.
            let mut window_start=start;let mut extra_y=0.;let mut extra_x=0.;
            let partial_logical_line=self.rope.line(first.min(self.rope.len_lines()-1)).len_chars()>MAX_WINDOW_CHARS;
            if partial_logical_line{
                let line_start=self.rope.line_to_char(first);let chars_per_row=(width/(font*0.61)).max(1.)as usize;
                let budget=if wrap{((viewport.height()+OVERSCAN*2.)/(font*1.3)).ceil()as usize*chars_per_row}else{chars_per_row*4};
                let row=((viewport.min.y-self.line_heights.prefix(first))/(font*1.3).ceil()).max(0.)as usize;
                let target=self.jump.or(precise_byte).map(|byte|self.rope.byte_to_char(byte.min(self.rope.len_bytes()))).unwrap_or_else(||if wrap{line_start+row.saturating_sub(5)*chars_per_row}else{line_start+(viewport.min.x/(font*0.61)).max(0.)as usize});
                window_start=target.saturating_sub(chars_per_row*2).max(line_start).min(end.saturating_sub(budget));end=(window_start+budget.min(MAX_WINDOW_CHARS)).min(end);
                if wrap{extra_y=(window_start-line_start)as f32/chars_per_row as f32*(font*1.3).ceil();}else{extra_x=(window_start-line_start)as f32*font*0.61;ui.set_min_width(self.rope.line(first).len_chars()as f32*font*0.61);}
                last=first+1;
            }
            if self.ime_composing&&!self.last_window.is_empty(){window_start=self.last_window.start.min(self.rope.len_chars());end=self.last_window.end.min(self.rope.len_chars());}
            if end<window_start{end=window_start;}
            let old_window_start=window_start;
            let start_byte=self.rope.char_to_byte(window_start);
            if start_byte<document.text.len() {window_start=self.rope.byte_to_char(crate::reading_precision::grapheme_range(&document.text,start_byte).start);}
            if end<self.rope.len_chars() {
                let end_byte=self.rope.char_to_byte(end);let boundary=crate::reading_precision::grapheme_range(&document.text,end_byte);
                if boundary.start!=end_byte {end=self.rope.byte_to_char(boundary.end);}
            }
            if partial_logical_line && window_start!=old_window_start {
                let delta=window_start as f32-old_window_start as f32;
                if wrap {let chars_per_row=(width/(font*0.61)).max(1.);extra_y+=delta/chars_per_row*(font*1.3).ceil();}
                else {extra_x+=delta*font*0.61;}
            }
            let mut window=self.rope.slice(window_start..end).to_string();let before_window=window.clone();
            let y=self.line_heights.prefix(first)+extra_y;let min=ui.max_rect().min+egui::vec2(extra_x,y);
            let child_rect=egui::Rect::from_min_size(min,egui::vec2(width,(total-y).max(1.)));
            let global_before=global;
            let mut local_state=self.kernel_state.clone();local_state.set_undoer(egui::util::undoer::Undoer::with_settings(egui::util::undoer::Settings{max_undos:0,..Default::default()}));
            let local=cursor_range(egui::text::CCursor::new(global.primary.index.saturating_sub(window_start).min(end-window_start)),egui::text::CCursor::new(global.secondary.index.saturating_sub(window_start).min(end-window_start)));
            local_state.cursor.set_char_range(Some(local));local_state.store(ui.ctx(),id);
            let mut output=None;
            ui.scope_builder(egui::UiBuilder::new().max_rect(child_rect).layout(egui::Layout::top_down(egui::Align::Min)),|ui|{
                let mut layouter=|ui:&egui::Ui,text:&dyn egui::TextBuffer,_:f32|{
                    let started=Instant::now();let galley=ui.fonts_mut(|fonts|fonts.layout(text.as_str().to_owned(),egui::FontId::monospace(font),ui.visuals().text_color(),if wrap{width}else{f32::INFINITY}));
                    crate::reading_state::layout_sample(&source_layout_metrics,"virtual-source-font-layout",started);galley
                };
                output=Some(egui::TextEdit::multiline(&mut window).id(id).font(egui::FontId::monospace(font)).frame(false).margin(egui::Vec2::ZERO).desired_width(if wrap{width}else{f32::INFINITY}).desired_rows(1).lock_focus(true).layouter(&mut layouter).show(ui));
            });
            let output=output.unwrap();
            self.kernel_state=output.state.clone();
            self.last_galley=Some(output.galley.clone());
            source_galleys.push(reading.geometry_cache.linear(output.galley.clone(),output.galley_pos,self.rope.char_to_byte(window_start)));
            if focus{output.response.request_focus();}
            self.source_rect=Some(egui::Rect::from_min_size(ui.max_rect().min+viewport.min.to_vec2(),egui::vec2(width,viewport.height())));
            self.visible_source=window_start..end;self.laid_out_lines=output.galley.rows.len();
            // Global cursor survives window changes and offscreen selection anchors.
            let pointer_pressed=ui.input(|i|i.pointer.primary_pressed());let pointer_down=ui.input(|i|i.pointer.primary_down());
            let local_after=output.cursor_range.unwrap_or(local);
            if pointer_pressed&&output.response.hovered(){self.pending_drag_anchor=Some(window_start+local_after.secondary.index);self.pending_external_selection=false;}
            let keyboard=output.response.has_focus()&&ui.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Text(_)|egui::Event::Paste(_)|egui::Event::Ime(egui::ImeEvent::Commit(_)|egui::ImeEvent::Preedit(_))|egui::Event::Key{pressed:true,..})));
            if (pointer_down&&self.pending_drag_anchor.is_some())||(keyboard&&!self.pending_external_selection&&(local_after.primary.index!=local.primary.index||local_after.secondary.index!=local.secondary.index))||output.response.changed(){
                global=cursor_range(egui::text::CCursor::new(window_start+local_after.primary.index),egui::text::CCursor::new(if pointer_down{self.pending_drag_anchor.unwrap_or(window_start+local_after.secondary.index)}else{window_start+local_after.secondary.index}));
                self.pending_external_selection=false;
            }else if pointer_pressed&&output.response.hovered(){global=cursor_range(egui::text::CCursor::new(window_start+local_after.primary.index),egui::text::CCursor::new(window_start+local_after.secondary.index));}
            if !pointer_down{self.pending_drag_anchor=None;}
            if pointer_down&& let(Some(anchor),Some(pointer))=(self.pending_drag_anchor,ui.input(|i|i.pointer.interact_pos())){global=cursor_range(egui::text::CCursor::new(window_start+output.galley.cursor_from_pos(pointer-output.galley_pos).index),egui::text::CCursor::new(anchor));}
            if output.response.changed(){
                let(a,b,c)=diff(&before_window,&window);let byte_start=self.rope.char_to_byte(window_start);
                let mut undoer=state.undoer();undoer.feed_state(ui.input(|i|i.time)-0.001,&(global_before,document.text.clone()));
                self.replace(byte_start+a..byte_start+b,&window[a..c]);document.text.replace_range(byte_start+a..byte_start+b,&window[a..c]);document.mark_text_changed();self.document_revision=document.revision();
                undoer.feed_state(ui.input(|i|i.time),&(global,document.text.clone()));state.set_undoer(undoer);changed=true;
            }
            // Measure each logical line from native glyph rows; no estimates after visiting.
            if !partial_logical_line&&extra_y==0.&&last-first==before_window.bytes().filter(|b|*b==b'\n').count()+usize::from(!before_window.ends_with('\n')){
                let mut logical=first;let mut height=0.;for row in &output.galley.rows{height+=row.height();if row.ends_with_newline||std::ptr::eq(row,output.galley.rows.last().unwrap()){if logical<self.line_heights.len(){let measured=height.max(font);if (self.line_heights.value(logical)-measured).abs()>0.1{self.line_height_stamp=self.line_height_stamp.wrapping_add(1);}self.line_heights.set(logical,measured);if self.measured_line_heights.len()<10000||self.measured_line_heights.contains_key(&logical){self.measured_line_heights.insert(logical,measured);}}logical+=1;height=0.;}}
            }
            if let Some(byte)=self.jump.take(){let char_index=self.rope.byte_to_char(byte.min(self.rope.len_bytes()));if char_index>=window_start&&char_index<=end{let rect=output.galley.pos_from_cursor(egui::text::CCursor::new(char_index-window_start)).translate(output.galley_pos.to_vec2());ui.scroll_to_rect_animation(rect,Some(egui::Align::TOP),egui::style::ScrollAnimation::none());}}
            self.last_window=window_start..end;
            state.cursor.set_char_range(Some(global));state.store(ui.ctx(),id);
            self.selection=[global.primary.index,global.secondary.index];
            if std::env::var_os("MDCARD_PERFORMANCE_REPORT").is_some()&&!entry_events.is_empty(){if self.selection_trace.len()>=32{self.selection_trace.pop_front();}self.selection_trace.push_back(serde_json::json!({"frame":self.frame_count,"entry":entry_selection,"exit":self.selection,"window":[window_start,end],"keyboard":keyboard,"bridge":self.pending_external_selection,"events":entry_events}));}
            response=Some(output.response);
        });
        reading.viewport(reading_key, out.inner_rect);
        let precise = reading.geometry_frame(
            document,
            reading_key,
            &source_galleys,
            out.inner_rect,
            [out.state.offset.x, out.state.offset.y],
            [
                (out.content_size.x - out.inner_rect.width()).max(0.),
                (out.content_size.y - out.inner_rect.height()).max(0.),
            ],
            ui.ctx(),
        );
        if precise.is_some() {
            reading_target = precise;
        }
        if let Some(target) = reading_target {
            let started = Instant::now();
            out.state.offset = egui::vec2(target[0], target[1]);
            out.state.store(ui.ctx(), out.id);
            reading.commit_sample(started);
        }
        let offset = out.state.offset.y;
        if let Some(target) = reading_target {
            reading.target(reading_key, target, ui.ctx());
            self.sync_from_source = false;
            self.sync_from_preview = false;
        }
        let line = self
            .line_heights
            .at(offset)
            .min(self.rope.len_lines().saturating_sub(1));
        if reading_target.is_none() || !reading.restoring(reading_key) {
            if reading.needs_calibration(reading_key, self.line_height_stamp) {
                reading.calibration(
                    document,
                    reading_key,
                    &reading_layout,
                    self.line_height_stamp,
                    self.measured_line_heights
                        .iter()
                        .map(|(line, height)| (*line as u64, *height))
                        .collect(),
                );
            }
            reading.observe(
                document,
                reading_key,
                reading_layout,
                [out.state.offset.x, offset],
                [out.inner_rect.width(), out.inner_rect.height()],
                self.rope.line_to_byte(line),
                ((offset - self.line_heights.prefix(line)) / self.line_heights.value(line).max(1.))
                    .clamp(0., 1.),
                0.,
                out.inner_rect
                    .contains(ui.ctx().pointer_hover_pos().unwrap_or_default()),
            );
        }
        reading.store_selection(document, reading_key, self.selection);
        if std::env::var_os("MDCARD_PERFORMANCE_REPORT").is_some() {
            self.source_scrollbar = ui.ctx().read_response(out.id.with(1usize)).map(|r| r.rect);
        }
        self.last_source_horizontal_offset = out.state.offset.x;
        if (offset - self.last_source_offset).abs() > 0.5 {
            self.last_source_offset = offset;
            let line = self.line_heights.at(offset);
            self.source_anchor = self.rope.line_to_byte(line.min(self.rope.len_lines() - 1));
            if !driven_by_preview && reading_target.is_none() {
                self.sync_from_source = true;
            }
        }
        (response.unwrap(), changed)
    }
    pub fn show_plain_preview(
        &mut self,
        ui: &mut egui::Ui,
        font: f32,
        document: &crate::Document,
        reading: &mut crate::reading_state::ReadingState,
        reading_key: &str,
        open_generation: u64,
    ) {
        let height = (font * 1.3).ceil();
        let mut area = egui::ScrollArea::both()
            .id_salt(("preview-scroll", document.id, open_generation))
            .auto_shrink([false; 2]);
        let layout = format!("plain-preview:{:.1}:{font}", ui.available_width());
        reading.layer(reading_key, ui.layer_id());
        let restore = reading.plan(
            document,
            reading_key,
            open_generation,
            &layout,
            ui.ctx(),
            ui.available_rect_before_wrap(),
            self.sync_from_source,
        );
        let mut target = restore.as_ref().map(|restore| {
            if restore.exact {
                restore.position.offset
            } else {
                let byte = crate::reading_state::byte_floor(
                    &document.text,
                    restore
                        .position
                        .text_anchor
                        .as_ref()
                        .map_or(restore.position.anchor.byte, |t| t.source.source_byte),
                );
                [
                    restore.position.offset[0],
                    (self.rope.byte_to_line(byte) as f32 * height
                        + restore.position.anchor.fraction * height
                        - restore.position.anchor.viewport_y)
                        .max(0.),
                ]
            }
        });
        target = target.map(|target| reading.scroll_target(reading_key, target));
        if let Some(target) = target {
            area = area
                .horizontal_scroll_offset(target[0])
                .vertical_scroll_offset(target[1]);
            self.sync_from_source = false;
            self.sync_from_preview = false;
        }
        if self.sync_from_source {
            area = area
                .vertical_scroll_offset(self.rope.byte_to_line(self.source_anchor) as f32 * height);
            self.sync_from_source = false;
        }
        let width = ui.available_width();
        let mut galleys = Vec::new();
        let mut out = area.show_viewport(ui, |ui, viewport| {
            ui.set_min_height(self.rope.len_lines() as f32 * height);
            let first = (viewport.min.y / height).max(0.) as usize;
            let last = ((viewport.max.y / height).ceil() as usize + 3).min(self.rope.len_lines());
            self.preview_mounted = last - first;
            self.preview_range = first..last;
            let origin = ui.max_rect().min;
            for index in first..last {
                let line = self.rope.line(index);
                let start = (viewport.min.x / (font * 0.61)).max(0.) as usize;
                let end =
                    (start + (width / (font * 0.61)).ceil() as usize + 10).min(line.len_chars());
                let start = start.min(end);
                let line_byte = self.rope.line_to_byte(index);
                let start_byte = line_byte + line.char_to_byte(start);
                let snapped = crate::reading_precision::grapheme_range(&document.text, start_byte)
                    .start
                    .max(line_byte);
                let start = line.byte_to_char(snapped - line_byte);
                let mut end = end;
                if end < line.len_chars() {
                    let boundary = crate::reading_precision::grapheme_range(
                        &document.text,
                        line_byte + line.char_to_byte(end),
                    );
                    if boundary.start != line_byte + line.char_to_byte(end) {
                        end = line.byte_to_char(
                            boundary.end.min(line_byte + line.len_bytes()) - line_byte,
                        );
                    }
                }
                let value = line.slice(start..end).to_string();
                ui.set_min_width(line.len_chars() as f32 * font * 0.61);
                let layout_started = Instant::now();
                let galley = ui.fonts_mut(|f| {
                    f.layout_no_wrap(
                        value,
                        egui::FontId::monospace(font),
                        ui.visuals().text_color(),
                    )
                });
                crate::reading_state::layout_sample(
                    &reading.layout_metrics,
                    "virtual-plain-font-layout",
                    layout_started,
                );
                let pos = origin + egui::vec2(start as f32 * font * 0.61, index as f32 * height);
                ui.painter()
                    .galley(pos, galley.clone(), ui.visuals().text_color());
                galleys.push(reading.geometry_cache.linear(
                    galley,
                    pos,
                    self.rope.line_to_byte(index) + line.char_to_byte(start),
                ));
            }
        });
        reading.viewport(reading_key, out.inner_rect);
        let precise = reading.geometry_frame(
            document,
            reading_key,
            &galleys,
            out.inner_rect,
            [out.state.offset.x, out.state.offset.y],
            [
                (out.content_size.x - out.inner_rect.width()).max(0.),
                (out.content_size.y - out.inner_rect.height()).max(0.),
            ],
            ui.ctx(),
        );
        if precise.is_some() {
            target = precise;
        }
        if let Some(target) = target {
            let started = Instant::now();
            out.state.offset = egui::vec2(target[0], target[1]);
            out.state.store(ui.ctx(), out.id);
            reading.commit_sample(started);
        }
        if let Some(target) = target {
            reading.target(reading_key, target, ui.ctx());
        }
        if target.is_none() || !reading.restoring(reading_key) {
            let line = (out.state.offset.y / height) as usize;
            let line = line.min(self.rope.len_lines().saturating_sub(1));
            reading.observe(
                document,
                reading_key,
                layout,
                [out.state.offset.x, out.state.offset.y],
                [out.inner_rect.width(), out.inner_rect.height()],
                self.rope.line_to_byte(line),
                (out.state.offset.y / height).fract(),
                0.,
                out.inner_rect
                    .contains(ui.ctx().pointer_hover_pos().unwrap_or_default()),
            );
        }
        if (out.state.offset.y - self.last_preview_offset).abs() > 0.5 {
            self.last_preview_offset = out.state.offset.y;
            self.preview_anchor = self
                .rope
                .line_to_byte((out.state.offset.y / height) as usize);
            self.sync_from_preview = true;
        }
    }
}

pub fn diff(before: &str, after: &str) -> (usize, usize, usize) {
    let mut start = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !before.is_char_boundary(start) || !after.is_char_boundary(start) {
        start -= 1;
    }
    let suffix = before[start..]
        .bytes()
        .rev()
        .zip(after[start..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut old_end = before.len() - suffix;
    let mut new_end = after.len() - suffix;
    while !before.is_char_boundary(old_end) || !after.is_char_boundary(new_end) {
        old_end += 1;
        new_end += 1;
    }
    (start, old_end, new_end)
}
fn bounded_push(queue: &mut VecDeque<f64>, sample: f64) {
    if queue.len() >= 2048 {
        queue.pop_front();
    }
    queue.push_back(sample);
}
fn ui_clip_height(ui: &egui::Ui) -> f32 {
    ui.clip_rect().height()
}
fn cursor_range(
    primary: egui::text::CCursor,
    secondary: egui::text::CCursor,
) -> egui::text::CCursorRange {
    egui::text::CCursorRange {
        primary,
        secondary,
        h_pos: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reopening_same_wrapped_layout_does_not_measure_a_temporary_top_before_its_index() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.25);
        crate::localization::install_fonts(&ctx, crate::localization::Language::English);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wrapped.txt");
        let text = (0..2400).map(|i| format!("纯文本 {i:05} text-{i:05}：{}\n", "中文🙂 é 👩🏽‍💻 👍🏿 literal **source** [label](url) a long logical line wraps into different visual rows. ".repeat(4))).collect::<String>();
        std::fs::write(&path, &text).unwrap();
        let mut doc = crate::Document::open(&path).unwrap();
        let mut reading = crate::reading_state::ReadingState::default();
        reading.bind(&mut doc, "secondary", 1);
        // Arrange the valid history from the observed font-reflow case. These
        // are public recording calls, not an artificial worker delay or GUI.
        let deadline = Instant::now() + std::time::Duration::from_secs(3);
        loop {
            reading.byte_at_char(&doc, 0);
            let tasks = reading.report(&doc, None)["background_tasks"].clone();
            if tasks
                .as_array()
                .unwrap()
                .iter()
                .all(|task| task["finished_monotonic_ns"].as_u64().unwrap_or(0) > 0)
            {
                break;
            }
            assert!(Instant::now() < deadline, "index did not finish");
            std::thread::yield_now();
        }
        reading.byte_at_char(&doc, 0);
        let width = 405.40625;
        let layout = format!("large-source:{width:.1}:14.6:true");
        reading.observe(
            &doc,
            "secondary/source",
            layout.clone(),
            [0., 270.242_13],
            [width, 586.],
            534,
            0.4338878,
            0.,
            true,
        );
        reading.calibration(
            &doc,
            "secondary/source",
            &layout,
            1,
            (1..=7).map(|line| (line, 184.9375)).collect(),
        );
        reading.bind(&mut doc, "secondary", 2);
        let mut old = LargeDocument::default();
        let mut frame = 0;
        let mut draw = |large: &mut LargeDocument,
                        doc: &mut crate::Document,
                        reading: &mut crate::reading_state::ReadingState,
                        generation| {
            large.ensure(&doc.text, generation, doc.revision(), &ctx);
            frame += 1;
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(frame as f64 / 60.),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 586.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ctx, |ui| {
                            large.show_source(
                                ui,
                                doc,
                                egui::Id::new(("second-editor", generation)),
                                14.6,
                                true,
                                false,
                                reading,
                                "secondary/source",
                                generation,
                            );
                        });
                },
            );
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            draw(&mut old, &mut doc, &mut reading, 2);
            let view = reading.report(&doc, None)["views"]["secondary/source"].clone();
            if view["pending"].is_null() && view["geometry"]["logical_line"] == 1 {
                break;
            }
            assert!(Instant::now() < deadline, "{view}");
            std::thread::yield_now();
        }
        let before = reading.report(&doc, None)["views"]["secondary/source"].clone();
        assert_eq!(before["geometry"]["logical_line"], 1);
        assert!(!old.measured_line_heights.contains_key(&0));
        let mut reopened = crate::Document::open(&path).unwrap();
        reading.bind(&mut reopened, "secondary", 3);
        assert_eq!(reopened.id, doc.id);
        assert_ne!(reopened.buffer_id(), doc.buffer_id());
        let mut next = LargeDocument::default();
        draw(&mut next, &mut reopened, &mut reading, 3);
        let temporary_top_was_measured = next.measured_line_heights.contains_key(&0);
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            draw(&mut next, &mut reopened, &mut reading, 3);
            let after = reading.report(&reopened, None)["views"]["secondary/source"].clone();
            if after["pending"].is_null()
                && after["geometry"]["source_byte"] == before["geometry"]["source_byte"]
            {
                for _ in 0..12 {
                    draw(&mut next, &mut reopened, &mut reading, 3);
                }
                let after = reading.report(&reopened, None)["views"]["secondary/source"].clone();
                let dy = (after["position"]["offset"][1].as_f64().unwrap()
                    - before["position"]["offset"][1].as_f64().unwrap())
                .abs();
                let dx = (after["position"]["offset"][0].as_f64().unwrap()
                    - before["position"]["offset"][0].as_f64().unwrap())
                .abs();
                assert!(
                    !temporary_top_was_measured,
                    "loading the history index mounted unrelated top: before={before} after={after}"
                );
                assert!(
                    dx <= 2. && dy <= 2.,
                    "identical-layout offset drift ({dx},{dy}): before={before} after={after}"
                );
                assert!(
                    (after["geometry"]["viewport_y"].as_f64().unwrap()
                        - before["geometry"]["viewport_y"].as_f64().unwrap())
                    .abs()
                        <= 2.
                );
                break;
            }
            assert!(Instant::now() < deadline, "{after}");
            std::thread::yield_now();
        }
    }
    #[test]
    fn a_mounted_source_keeps_text_input_during_worker_clone_rebinding() {
        for events in [
            vec![egui::Event::Text("新增🙂".into())],
            vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit("临时".into())),
                egui::Event::Ime(egui::ImeEvent::Commit("新增🙂".into())),
            ],
        ] {
            let ctx = egui::Context::default();
            let id = egui::Id::new(("second-editor", 1u64));
            let mut doc =
                crate::Document::from_text(&"A long editable logical line 中文🙂.\n".repeat(9000));
            let mut reading = crate::reading_state::ReadingState::default();
            reading.bind(&mut doc, "secondary", 1);
            let mut large = LargeDocument::default();
            let deadline = Instant::now() + std::time::Duration::from_secs(5);
            loop {
                large.ensure(&doc.text, 1, doc.revision(), &ctx);
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        large.show_source(
                            ui,
                            &mut doc,
                            id,
                            15.,
                            true,
                            false,
                            &mut reading,
                            "secondary/source",
                            1,
                        );
                    });
                });
                let view = reading.report(&doc, None)["views"]["secondary/source"].clone();
                if !view["position"].is_null() && !large.last_window.is_empty() {
                    break;
                }
                assert!(Instant::now() < deadline, "{view}");
                std::thread::yield_now();
            }
            let mut cloned = doc.clone();
            assert_ne!(cloned.buffer_id(), doc.buffer_id());
            reading.store_selection(&cloned, "secondary/source", [4, 4]);
            reading.rebind_after_commit(&mut cloned, "secondary", 1);
            assert!(reading.awaiting_initial_source_index(&cloned, "secondary/source"));
            assert!(!large.last_window.is_empty());
            ctx.memory_mut(|memory| memory.request_focus(id));
            let original = cloned.text.clone();
            let mut changed = false;
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    large.ensure(&cloned.text, 1, cloned.revision(), ctx);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        changed = large
                            .show_source(
                                ui,
                                &mut cloned,
                                id,
                                15.,
                                true,
                                false,
                                &mut reading,
                                "secondary/source",
                                1,
                            )
                            .1;
                    });
                },
            );
            let mut expected = original;
            expected.insert_str(4, "新增🙂");
            assert!(changed, "a mounted editor must process its real Text event");
            assert_eq!(cloned.text, expected);
            assert_eq!(cloned.revision(), doc.revision() + 1);
        }
    }
    #[test]
    fn offscreen_selection_is_not_clamped_back_by_unrelated_keyboard_events() {
        let ctx = egui::Context::default();
        let mut large = LargeDocument::default();
        let mut doc = crate::Document::from_text(
            &"# Title\n\n中文🙂 source paragraph with retained cursor.\n\n".repeat(3000),
        );
        large.ensure(&doc.text, 1, doc.revision(), &ctx);
        let id = egui::Id::new(("source-editor", 1u64));
        let mut state = egui::text_edit::TextEditState::default();
        state
            .cursor
            .set_char_range(Some(crate::reading_state::cursor([14, 0])));
        state.store(&ctx, id);
        large.jump = Some(large.rope.char_to_byte(12_000));
        let mut reading = crate::reading_state::ReadingState::default();
        for frame in 0..4 {
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900., 700.),
                )),
                ..egui::RawInput::default()
            };
            if frame == 1 {
                input.events.push(egui::Event::Key {
                    key: egui::Key::P,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                });
            }
            if frame == 2 {
                ctx.memory_mut(|m| m.surrender_focus(id));
                input.events.push(egui::Event::Text("b.md".into()));
            }
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    large.show_source(
                        ui,
                        &mut doc,
                        id,
                        15.,
                        true,
                        frame == 0,
                        &mut reading,
                        "main/source",
                        1,
                    );
                });
            });
            assert_eq!(
                large.selection,
                [14, 0],
                "unrelated keyboard event must not remap the local clamped selection"
            );
        }
        assert!(
            large.visible_source.start > 1000,
            "fixture must leave the saved selection outside the virtual source window"
        );
    }
    #[test]
    fn heights_updates_and_offset_lookup() {
        let mut h = Heights::new(vec![10., 20., 30.]);
        assert_eq!(h.total(), 60.);
        assert_eq!(h.at(10.), 1);
        assert_eq!(h.at(59.), 2);
        h.set(1, 100.);
        assert_eq!(h.prefix(2), 110.);
        assert_eq!(h.at(109.), 1);
        assert_eq!(h.at(110.), 2);
    }
    #[test]
    fn diff_preserves_unicode_boundaries() {
        for (a, b) in [
            ("你好🦀abc", "你呀🦀abc"),
            ("中文", "中文🦀"),
            ("🦀abc", "abc"),
            ("abc", "abc"),
        ] {
            let (s, e, n) = diff(a, b);
            let mut result = a.to_owned();
            result.replace_range(s..e, &b[s..n]);
            assert_eq!(result, b);
        }
    }
    #[test]
    fn virtual_pages_keep_verified_original_text_coordinates() {
        let long = format!(
            "**{}**",
            (0..900)
                .map(|i| format!(
                    "word-{i:04} 中文 e\u{301} 👩🏽‍💻 [link](https://invalid.test) &amp; "
                ))
                .collect::<String>()
        );
        let table = format!(
            "| Heading | Value |\n| --- | --- |\n{}",
            (0..600)
                .map(|i| format!("| row-{i:04} | body-{i:04} |\n"))
                .collect::<String>()
        );
        for source in [long, table] {
            let pages = semantic_pages(&source);
            assert!(pages.len() > 1);
            let maps = page_source_maps(&source, &[], &pages);
            assert_eq!(pages.len(), maps.len());
            assert!(
                maps.iter().all(|m| !m.is_empty()),
                "all semantic pages need proven provenance"
            );
            let mut checked = 0;
            for ((_, page, _), map) in pages.iter().zip(&maps) {
                for span in map {
                    if span.linear {
                        assert_eq!(&page[span.preview.clone()], &source[span.source.clone()]);
                        checked += span.source.len();
                    }
                }
            }
            assert!(
                checked > source.len() / 2,
                "not merely first-page provenance: {checked} / {}",
                source.len()
            );
        }
    }
    #[test]
    fn semantic_pages_preserve_cross_page_emphasis_links_and_nested_code() {
        use pulldown_cmark::{Event, Tag};
        let paragraph = format!(
            "**{} [link](https://example.com) {}**",
            "strong 中文 ".repeat(900),
            "more strong ".repeat(900).trim_end()
        );
        let pages = semantic_pages(&paragraph);
        assert!(pages.len() > 3);
        let mut observed = String::new();
        let mut links = 0;
        for (_, page, _) in &pages {
            let mut strong = false;
            for event in pulldown_cmark::Parser::new_ext(page, pulldown_cmark::Options::all()) {
                match event {
                    Event::Start(Tag::Strong) => strong = true,
                    Event::Text(t) => {
                        assert!(
                            strong,
                            "style context lost at a virtual page boundary: {:?} ... {:?}",
                            &page[..page.len().min(80)],
                            &page[page.len().saturating_sub(80)..]
                        );
                        observed.push_str(&t)
                    }
                    Event::Start(Tag::Link { dest_url, .. }) => {
                        assert_eq!(dest_url.as_ref(), "https://example.com");
                        links += 1;
                    }
                    _ => {}
                }
            }
        }
        let expected = pulldown_cmark::Parser::new_ext(&paragraph, pulldown_cmark::Options::all())
            .filter_map(|e| {
                if let Event::Text(t) = e {
                    Some(t.to_string())
                } else {
                    None
                }
            })
            .collect::<String>();
        assert_eq!(observed, expected);
        assert_eq!(links, 1);
        let code = "/* $literal$ stays code */ let x = 1;\n".repeat(700);
        let nested = format!(
            "> 7. ```rust\n{}>    ```\n",
            code.lines()
                .map(|line| format!(">    {line}\n"))
                .collect::<String>()
        );
        let parsed = rich_nodes::parse(&nested);
        assert!(parsed.1.is_empty());
        let pages = semantic_pages(&parsed.0);
        let body = pages
            .iter()
            .filter_map(|(_, _, p)| p.as_ref())
            .map(|p| p.source[p.bytes.clone()].to_string())
            .collect::<String>();
        assert_eq!(
            body, code,
            "nested code pages must use parser-decoded body, never synthetic wrapper offsets"
        );
        assert!(
            pages
                .iter()
                .filter_map(|(_, _, p)| p.as_ref())
                .all(|p| p.language == "rust" && p.bytes.len() <= 8192)
        );
    }
    #[test]
    fn paginated_tables_keep_headers_and_every_body_cell() {
        use pulldown_cmark::{Event, Tag, TagEnd};
        let source = format!(
            "| Name | Value |\n| --- | ---: |\n{}",
            (0..700)
                .map(|i| format!("| row-{i:04} | data-{i:04}-中文 |\n"))
                .collect::<String>()
        );
        let pages = semantic_pages(&source);
        assert!(pages.len() > 3);
        let mut body = Vec::new();
        for (_, text, _) in pages {
            let mut head = false;
            let mut tables = 0;
            for event in pulldown_cmark::Parser::new_ext(&text, pulldown_cmark::Options::all()) {
                match event {
                    Event::Start(Tag::Table(_)) => tables += 1,
                    Event::Start(Tag::TableHead) => head = true,
                    Event::End(TagEnd::TableHead) => head = false,
                    Event::Text(value) if !head => body.push(value.to_string()),
                    _ => {}
                }
            }
            assert_eq!(
                tables, 1,
                "each virtual table page must be valid Markdown with a header"
            );
        }
        assert_eq!(body.len(), 1400);
        assert_eq!(body[0], "row-0000");
        assert_eq!(body.last().unwrap(), "data-0699-中文");
    }
    #[test]
    fn source_selection_direction_survives_window_mapping() {
        let selection = cursor_range(egui::text::CCursor::new(1500), egui::text::CCursor::new(0));
        assert_eq!(selection.primary.index, 1500);
        assert_eq!(selection.secondary.index, 0);
        let reverse = cursor_range(selection.secondary, selection.primary);
        assert_eq!(reverse.primary.index, 0);
        assert_eq!(reverse.secondary.index, 1500);
    }
    #[test]
    fn reference_links_use_global_definitions_without_copying_unrelated_ones() {
        let defs = "[target]: https://example.com \"Title\"\n[unused]: https://unused.example\n";
        let parser = pulldown_cmark::Parser::new_ext(defs, pulldown_cmark::Options::all());
        let resolved = resolve_references(
            "See [visible][target] and ![image][target].",
            defs,
            parser.reference_definitions(),
        );
        assert!(resolved.contains("[target]:"));
        assert!(!resolved.contains("unused"));
        assert_eq!(
            pulldown_cmark::Parser::new_ext(&resolved, pulldown_cmark::Options::all())
                .filter(|e| matches!(
                    e,
                    pulldown_cmark::Event::Start(
                        pulldown_cmark::Tag::Link { .. } | pulldown_cmark::Tag::Image { .. }
                    )
                ))
                .count(),
            2
        );
    }
    #[test]
    fn footnote_reference_event_survives_separate_viewer_partitions() {
        let events = pulldown_cmark::Parser::new_ext(
            "An isolated reference[^note].",
            pulldown_cmark::Options::all(),
        )
        .collect::<Vec<_>>();
        assert!(events.iter().any(|event|matches!(event,pulldown_cmark::Event::FootnoteReference(id)if id.as_ref()=="note")));
    }
}
