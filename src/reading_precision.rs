// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Pure source anchors. Only size-bounded source fragments are persisted, not
//! unbounded version snapshots. Small nodes can fit entirely in those fragments.
//! GUI layout geometry is observed separately from this API.
use serde::{Deserialize, Serialize};
use std::ops::Range;
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Anchor {
    pub source_byte: usize,
    pub grapheme_range: Range<usize>,
    pub grapheme: String,
    pub before: String,
    pub after: String,
    pub source_fingerprint: u64,
    pub source_len: usize,
    pub proof: Option<Box<AnchorProof>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(not(test), allow(dead_code))] // Pure adapter API; Stage A GUI uses its cached-fingerprint capture variant.
pub struct Resolved {
    pub exact: bool,
    pub source_byte: usize,
    pub grapheme_range: Range<usize>,
    pub tier: String,
    pub evidence: serde_json::Value,
}

#[cfg_attr(not(test), allow(dead_code))] // Pure adapter API; Stage A GUI uses its cached-fingerprint capture variant.
pub fn fingerprint(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut state = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut state);
    state.finish()
}

pub fn grapheme_range(text: &str, byte: usize) -> Range<usize> {
    if text.is_empty() {
        return 0..0;
    }
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    let mut cursor = GraphemeCursor::new(byte, text.len(), true);
    let start = if cursor.is_boundary(text, 0).unwrap_or(false) && byte < text.len() {
        byte
    } else {
        cursor.prev_boundary(text, 0).ok().flatten().unwrap_or(0)
    };
    let mut end = GraphemeCursor::new(start, text.len(), true);
    start
        ..end
            .next_boundary(text, 0)
            .ok()
            .flatten()
            .unwrap_or(text.len())
}

#[cfg_attr(not(test), allow(dead_code))] // Pure adapter API; Stage A GUI uses its cached-fingerprint capture variant.
pub fn capture(text: &str, byte: usize) -> Anchor {
    enrich_with_fingerprint(text, byte, fingerprint(text))
}

pub fn capture_with_fingerprint(text: &str, byte: usize, source_fingerprint: u64) -> Anchor {
    let range = grapheme_range(text, byte);
    let before = text[..range.start]
        .char_indices()
        .rev()
        .nth(191)
        .map_or(0, |(b, _)| b);
    let after = range.end
        + text[range.end..]
            .char_indices()
            .nth(192)
            .map_or(text.len() - range.end, |(b, _)| b);
    Anchor {
        source_byte: range.start,
        grapheme_range: range.clone(),
        grapheme: if range.len() <= 1024 {
            text[range.clone()].to_owned()
        } else {
            String::new()
        },
        before: text[before..range.start].to_owned(),
        after: text[range.end..after].to_owned(),
        source_fingerprint,
        source_len: text.len(),
        proof: None,
    }
}

#[cfg_attr(not(test), allow(dead_code))] // Pure adapter API; Stage A GUI uses its cached-fingerprint capture variant.
fn resolve_unchanged(text: &str, anchor: &Anchor) -> Resolved {
    let same = text.len() == anchor.source_len && fingerprint(text) == anchor.source_fingerprint;
    let range = grapheme_range(text, anchor.source_byte);
    let exact = same
        && !range.is_empty()
        && range == anchor.grapheme_range
        && text[range.clone()] == anchor.grapheme;
    Resolved {
        exact,
        source_byte: range.start,
        grapheme_range: range,
        tier: if exact {
            "text"
        } else if text.is_empty() {
            "invalid"
        } else {
            "approximate"
        }
        .into(),
        evidence: serde_json::json!({"method":if exact {"unchanged-fingerprint"} else {"content-changed-stage-a"},"candidate_count":usize::from(exact)}),
    }
}

use std::collections::{BTreeMap, BTreeSet, VecDeque};
const MAX_BLOCKS: usize = 50_000;
const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
thread_local! {
    static WORK_CANCEL: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicU64>>> = const { std::cell::RefCell::new(None) };
    static WORK_PROGRESS: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicU64>>> = const { std::cell::RefCell::new(None) };
}
fn work_cancelled() -> bool {
    WORK_PROGRESS.with(|slot| {
        if let Some(progress) = slot.borrow().as_ref() {
            progress.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    });
    WORK_CANCEL.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|cancel| cancel.load(std::sync::atomic::Ordering::Acquire) != 0)
    })
}
struct CancellationScope(
    Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
    Option<std::sync::Arc<std::sync::atomic::AtomicU64>>,
);
impl CancellationScope {
    fn enter(
        cancel: std::sync::Arc<std::sync::atomic::AtomicU64>,
        progress: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> Self {
        Self(
            WORK_CANCEL.with(|slot| slot.replace(Some(cancel))),
            WORK_PROGRESS.with(|slot| slot.replace(Some(progress))),
        )
    }
}
impl Drop for CancellationScope {
    fn drop(&mut self) {
        WORK_CANCEL.with(|slot| {
            slot.replace(self.0.take());
        });
        WORK_PROGRESS.with(|slot| {
            slot.replace(self.1.take());
        });
    }
}

/// Cooperative work interruption. Cancellation never publishes partial proof.
pub fn enrich_cancelable(
    text: &str,
    byte: usize,
    source_fingerprint: u64,
    cancel: std::sync::Arc<std::sync::atomic::AtomicU64>,
    progress: std::sync::Arc<std::sync::atomic::AtomicU64>,
) -> Option<Anchor> {
    let _scope = CancellationScope::enter(cancel, progress);
    if work_cancelled() {
        return None;
    }
    let anchor = enrich_with_fingerprint(text, byte, source_fingerprint);
    (!work_cancelled()).then_some(anchor)
}
pub fn resolve_cancelable(
    text: &str,
    anchor: &Anchor,
    cancel: std::sync::Arc<std::sync::atomic::AtomicU64>,
    progress: std::sync::Arc<std::sync::atomic::AtomicU64>,
) -> Option<Resolved> {
    let _scope = CancellationScope::enter(cancel, progress);
    if work_cancelled() {
        return None;
    }
    let result = resolve(text, anchor);
    (!work_cancelled()).then_some(result)
}

/// All fields are bounded source fragments, hashes, counts or relative ranges.
/// Complete small nodes can be retained within the fixed 8 KiB proof limit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnchorProof {
    proofs: Vec<Proof>,
    seeds: Vec<Seed>,
    heading: Vec<String>,
    global_witnesses: Vec<u64>,
    old_window_count: usize,
    block: Option<Proof>,
    neighbors: Vec<Neighbor>,
    scopes: Vec<Scope>,
    block_identity: Option<BlockIdentity>,
    #[serde(default)]
    bounded_block: Option<BoundedBlock>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Precision {
    Text,
    SourceLine,
    Block,
    Section,
    Approximate,
    Invalid,
}
impl Precision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::SourceLine => "source-line",
            Self::Block => "block",
            Self::Section => "section",
            Self::Approximate => "approximate",
            Self::Invalid => "invalid",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Proof {
    range: Range<usize>,
    count: usize,
    heading_count: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Seed {
    range: Range<usize>,
    count: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Neighbor {
    source_range: Range<usize>,
    block_range: Range<usize>,
    text: String,
    count: usize,
    #[serde(
        default,
        alias = "unique_prefixes",
        skip_serializing_if = "Vec::is_empty"
    )]
    unique_fragments: Vec<Seed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    adjacent_gap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    complete: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    complete_witnesses: Vec<Seed>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Scope {
    left: usize,
    right: usize,
    old_count: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct BlockIdentity {
    length: usize,
    hash: u64,
    target_offset: usize,
    old_count: usize,
    old_heading_count: usize,
    scope_counts: Vec<usize>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct BoundedBlock {
    source_range: Range<usize>,
    text: String,
    seeds: Vec<Seed>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Bundle {
    source: Anchor,
    window: String,
    target: Range<usize>,
    proofs: Vec<Proof>,
    seeds: Vec<Seed>,
    heading: Vec<String>,
    global_witnesses: Vec<u64>,
    old_window_count: usize,
    block: Option<Proof>,
    neighbors: Vec<Neighbor>,
    scopes: Vec<Scope>,
    block_identity: Option<BlockIdentity>,
    bounded_block: Option<BoundedBlock>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Decision {
    exact: bool,
    source_byte: usize,
    reason: String,
    candidates: Vec<usize>,
    evidence: serde_json::Value,
}
fn occurrences(text: &str, needle: &str, limit: usize) -> (Vec<usize>, bool) {
    if needle.is_empty() {
        return (vec![], false);
    }
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = text[from..].find(needle) {
        if work_cancelled() {
            return (Vec::new(), true);
        }
        let at = from + p;
        if out.len() == limit {
            return (out, true);
        }
        out.push(at);
        from = at + text[at..].chars().next().unwrap().len_utf8();
        if from >= text.len() {
            break;
        }
    }
    (out, false)
}
fn headings(text: &str) -> Vec<(usize, Vec<String>)> {
    let mut result = vec![(0, vec![])];
    let mut levels: Vec<(usize, String)> = vec![];
    let mut current = None;
    let mut title = String::new();
    for (e, r) in pulldown_cmark::Parser::new(text).into_offset_iter() {
        if work_cancelled() {
            return vec![(0, Vec::new())];
        }
        match e {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { level, .. }) => {
                current = Some((r.start, level as usize));
                title.clear()
            }
            pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t)
                if current.is_some() =>
            {
                title.push_str(&t)
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some((at, level)) = current.take() {
                    while levels.last().is_some_and(|p| p.0 >= level) {
                        levels.pop();
                    }
                    levels.push((level, format!("{:016x}", fingerprint(&title))));
                    result.push((at, levels.iter().map(|p| p.1.clone()).collect()));
                }
            }
            _ => {}
        }
    }
    result
}
fn path_at(h: &[(usize, Vec<String>)], byte: usize) -> &[String] {
    &h[h.partition_point(|p| p.0 <= byte).saturating_sub(1)].1
}
fn blocks(text: &str) -> Vec<Range<usize>> {
    pulldown_cmark::Parser::new(text)
        .into_offset_iter()
        .take_while(|_| !work_cancelled())
        .filter_map(|(e, r)| {
            matches!(
                e,
                pulldown_cmark::Event::Start(
                    pulldown_cmark::Tag::Paragraph
                        | pulldown_cmark::Tag::Heading { .. }
                        | pulldown_cmark::Tag::CodeBlock(_)
                )
            )
            .then_some(r)
        })
        .flat_map(|range| {
            // A plain-text file can be one Markdown paragraph. Bound its
            // matching nodes by actual logical lines, never visual rows.
            if text[range.clone()].contains('\n') {
                let mut offset = range.start;
                text[range]
                    .split_inclusive('\n')
                    .take(MAX_BLOCKS + 1)
                    .take_while(|_| !work_cancelled())
                    .filter_map(move |line| {
                        let start = offset;
                        offset += line.len();
                        (!line.trim().is_empty()).then_some(start..offset)
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![range]
            }
        })
        .take(MAX_BLOCKS + 1)
        .collect()
}
fn complete_short_heading(body: &str) -> bool {
    body.len() < 24
        && matches!(
            pulldown_cmark::Parser::new(body).into_offset_iter().next(),
            Some((pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { .. }), range))
                if range == (0..body.len())
        )
}
fn actual_heading_node(text: &str, range: &Range<usize>, hs: &[(usize, Vec<String>)]) -> bool {
    let at = hs.partition_point(|(start, _)| *start <= range.start);
    at > 0
        && hs[at - 1].0 == range.start
        && !hs[at - 1].1.is_empty()
        && complete_short_heading(&text[range.clone()])
}
fn neighbor_occurrences(
    text: &str,
    neighbor: &Neighbor,
    source_blocks: &[Range<usize>],
    hs: &[(usize, Vec<String>)],
) -> (Vec<usize>, bool) {
    let (positions, truncated) = occurrences(text, &neighbor.text, 65);
    if neighbor.text.len() >= 24 {
        return (positions, truncated);
    }
    // A short heading is a complete structural node, never an arbitrary
    // short substring. Recheck global uniqueness and actual new AST identity.
    if truncated || positions.len() != 1 || neighbor.source_range != neighbor.block_range {
        return (vec![], truncated);
    }
    let range = positions[0]..positions[0] + neighbor.text.len();
    if source_blocks.contains(&range) && actual_heading_node(text, &range, hs) {
        (positions, false)
    } else {
        (vec![], false)
    }
}
fn neighbor_scopes(
    text: &str,
    byte: usize,
    window: &str,
    window_start: usize,
    hs: &[(usize, Vec<String>)],
) -> (Vec<Neighbor>, Vec<Scope>) {
    let blocks = blocks(text);
    let target_node = blocks.iter().find(|r| r.contains(&byte));
    let mut neighbors = vec![];
    let mut left = vec![];
    let mut right = vec![];
    for (previous, candidates) in [
        (
            true,
            blocks
                .iter()
                .filter(|r| r.end <= byte)
                .rev()
                .take(4)
                .collect::<Vec<_>>(),
        ),
        (
            false,
            blocks
                .iter()
                .filter(|r| r.start > byte)
                .take(4)
                .collect::<Vec<_>>(),
        ),
    ] {
        for r in candidates {
            if work_cancelled() {
                break;
            }
            let body = &text[r.clone()];
            let short_heading = actual_heading_node(text, r, hs);
            let mut boundaries: Vec<usize> = body
                .grapheme_indices(true)
                .take_while(|_| !work_cancelled())
                .map(|p| p.0)
                .collect();
            boundaries.push(body.len());
            if boundaries.len() < 9 && !short_heading {
                continue;
            }
            let width = 64.min(boundaries.len() - 1);
            let middle = (boundaries.len() - 1 - width) / 2;
            let first = 0..boundaries[width];
            let last = boundaries[boundaries.len() - 1 - width]..body.len();
            let mid = boundaries[middle]..boundaries[middle + width];
            let choices = if previous {
                [last, mid, first]
            } else {
                [first, mid, last]
            };
            let complete = target_node.and_then(|target| {
                let adjacent = (r.end <= target.start && target.start - r.end <= 8)
                    || (target.end <= r.start && r.start - target.end <= 8);
                (adjacent && body.len() <= 4096).then(|| body.to_owned())
            });
            let choices: Vec<_> = choices
                .into_iter()
                .filter_map(|range| {
                    let end = boundaries
                        .partition_point(|b| *b <= range.end.min(range.start + 256))
                        .saturating_sub(1);
                    let range = range.start..boundaries[end];
                    (range.len() >= 24 || short_heading && range == (0..body.len()))
                        .then_some(range)
                })
                .map(|range| {
                    let (v, t) = occurrences(text, &body[range.clone()], 65);
                    (range, if t { 66 } else { v.len() })
                })
                .collect();
            let Some((range, count)) = choices
                .iter()
                .find(|(_, count)| *count == 1)
                .or_else(|| complete.as_ref().and_then(|_| choices.first()))
                .cloned()
            else {
                continue;
            };
            let fragment = &body[range.clone()];
            if fragment.len() > 256 {
                continue;
            }
            let index = neighbors.len();
            let mut complete_witnesses = Vec::new();
            if complete.is_some() {
                let end = body.trim_end().len();
                let start = body.len() - body.trim_start().len();
                if let (Ok(first), Ok(last)) = (
                    boundaries.binary_search(&start),
                    boundaries.binary_search(&end),
                ) && last.saturating_sub(first) >= 8
                {
                    let unique = |range: Range<usize>| {
                        let (positions, truncated) = occurrences(text, &body[range], 2);
                        !truncated && positions.len() == 1
                    };
                    if unique(start..end) {
                        // With a fixed end, adding original graphemes cannot
                        // create extra occurrences. Find the shortest proven
                        // unique prefix/suffix instead of inflating old DP input.
                        for prefix in [true, false] {
                            let mut lower = 8;
                            let mut upper = last - first;
                            let range_for = |count| {
                                if prefix {
                                    start..boundaries[first + count]
                                } else {
                                    boundaries[last - count]..end
                                }
                            };
                            while lower < upper && !work_cancelled() {
                                let middle = lower + (upper - lower) / 2;
                                if unique(range_for(middle)) {
                                    upper = middle;
                                } else {
                                    lower = middle + 1;
                                }
                            }
                            let range = range_for(lower);
                            if !work_cancelled()
                                && !complete_witnesses.iter().any(|s: &Seed| s.range == range)
                            {
                                complete_witnesses.push(Seed { range, count: 1 });
                            }
                        }
                    }
                }
            }
            if count != 1 && complete_witnesses.is_empty() {
                continue;
            }
            let mut unique_fragments = Vec::new();
            let mut points: Vec<_> = fragment.grapheme_indices(true).map(|(p, _)| p).collect();
            points.push(fragment.len());
            'widths: for width in [8, 12, 16] {
                let meaningful_end = points
                    .partition_point(|p| *p <= fragment.trim_end().len())
                    .saturating_sub(1);
                let Some(last) = meaningful_end.checked_sub(width) else {
                    continue;
                };
                let mut starts = vec![0, last, last / 2, last / 4, last * 3 / 4];
                starts.extend((0..=last).step_by(8));
                let mut visited = BTreeSet::new();
                for start in starts {
                    if !visited.insert(start) {
                        continue;
                    }
                    let piece = points[start]..points[start + width];
                    let (positions, truncated) = occurrences(text, &fragment[piece.clone()], 65);
                    if !truncated && positions.len() == 1 {
                        unique_fragments.push(Seed {
                            range: piece,
                            count: 1,
                        });
                    }
                    if unique_fragments.len() == 4 {
                        break 'widths;
                    }
                }
            }
            neighbors.push(Neighbor {
                source_range: r.start + range.start..r.start + range.end,
                block_range: r.clone(),
                text: fragment.to_string(),
                count,
                unique_fragments,
                adjacent_gap: target_node.and_then(|target| {
                    let at = blocks.partition_point(|node| node.start < r.start);
                    let gap = if r.end <= target.start {
                        blocks
                            .get(at + 1)
                            .and_then(|next| text.get(r.end..next.start))
                    } else if target.end <= r.start {
                        at.checked_sub(1)
                            .and_then(|previous| text.get(blocks[previous].end..r.start))
                    } else {
                        None
                    };
                    gap.filter(|s| s.len() <= 8 && s.trim().is_empty())
                        .map(str::to_owned)
                }),
                complete,
                complete_witnesses,
            });
            if previous {
                left.push(index)
            } else {
                right.push(index)
            }
            if if previous { left.len() } else { right.len() } == 2 {
                break;
            }
        }
    }
    let mut scopes = vec![];
    for l in left {
        for r in &right {
            let range = neighbors[l].block_range.end..neighbors[*r].block_range.start;
            let (v, t) = occurrences(text, window, 65);
            let target = byte - window_start;
            scopes.push(Scope {
                left: l,
                right: *r,
                old_count: if t {
                    66
                } else {
                    v.iter()
                        .filter(|at| range.contains(&(*at + target)))
                        .count()
                },
            })
        }
    }
    (neighbors, scopes)
}
// All scalar-aligned 16-scalar rolling windows, independent of byte alignment,
// paragraph movement and inserted prefix length. Store only bottom-k hashes.
fn shingles(text: &str, mut visit: impl FnMut(u64, Range<usize>)) {
    const BASE: u64 = 1000003;
    let power = BASE.wrapping_pow(16);
    let mut hash = 0u64;
    let mut q = VecDeque::new();
    for (ordinal, (at, c)) in text.char_indices().enumerate() {
        if ordinal % 1024 == 0 && work_cancelled() {
            break;
        }
        hash = hash.wrapping_mul(BASE).wrapping_add(c as u64 + 1);
        q.push_back((at, c));
        if q.len() > 16 {
            let (_, old) = q.pop_front().unwrap();
            hash = hash.wrapping_sub((old as u64 + 1).wrapping_mul(power));
        }
        if q.len() == 16 {
            visit(hash, q.front().unwrap().0..at + c.len_utf8())
        }
    }
}
fn capture_bundle(text: &str, byte: usize, source_fingerprint: u64) -> Option<Bundle> {
    let source = capture_with_fingerprint(text, byte, source_fingerprint);
    let window = format!("{}{}{}", source.before, source.grapheme, source.after);
    let target = source.before.len()..source.before.len() + source.grapheme.len();
    if work_cancelled() {
        return None;
    }
    let hs = headings(text);
    if work_cancelled() {
        return None;
    }
    let heading = path_at(&hs, source.source_byte).to_vec();
    let mut starts: Vec<usize> = window.grapheme_indices(true).map(|(i, _)| i).collect();
    starts.push(window.len());
    let i = starts.partition_point(|b| *b < target.start);
    let end = i + 1;
    let mut proofs = vec![];
    for (l, r) in [
        (192, 192),
        (96, 96),
        (64, 64),
        (32, 32),
        (16, 16),
        (8, 32),
        (32, 8),
        (4, 64),
        (64, 4),
        (2, 96),
        (96, 2),
    ] {
        if work_cancelled() {
            return None;
        }
        let a = starts[i.saturating_sub(l)];
        let b = starts[(end + r).min(starts.len() - 1)];
        if b - a < 24 || proofs.iter().any(|p: &Proof| p.range == (a..b)) {
            continue;
        }
        let (positions, truncated) = occurrences(text, &window[a..b], 65);
        let count = if truncated { 66 } else { positions.len() };
        let heading_count = positions
            .iter()
            .filter(|p| path_at(&hs, **p + target.start - a) == heading)
            .count();
        proofs.push(Proof {
            range: a..b,
            count,
            heading_count,
        });
    }
    let seeds = (0..starts.len().saturating_sub(32))
        .step_by(24)
        .take_while(|_| !work_cancelled())
        .map(|j| {
            let range = starts[j]..starts[j + 32];
            let (v, t) = occurrences(text, &window[range.clone()], 65);
            Seed {
                range,
                count: if t { 66 } else { v.len() },
            }
        })
        .collect();
    let mut witnesses = BTreeMap::new();
    let origin = source.source_byte - target.start;
    shingles(text, |hash, r| {
        if r.end > origin.saturating_sub(64) && r.start < origin + window.len() + 64 {
            return;
        }
        witnesses.entry(hash).or_insert(r.start);
        if witnesses.len() > 16 {
            witnesses.pop_last();
        }
    });

    if work_cancelled() {
        return None;
    }
    let original_start = source.source_byte - source.before.len();
    let block_start = text[..source.source_byte]
        .rfind("\n\n")
        .map_or(0, |p| p + 2);
    let block_end = text[source.source_byte..]
        .find("\n\n")
        .map_or(text.len(), |p| source.source_byte + p);
    let block = if block_start >= original_start && block_end <= original_start + window.len() {
        let range = block_start - original_start..block_end - original_start;
        let (v, truncated) = occurrences(text, &window[range.clone()], 65);
        let count = if truncated { 66 } else { v.len() };
        let heading_count = v
            .iter()
            .filter(|p| path_at(&hs, **p + target.start - range.start) == heading)
            .count();
        Some(Proof {
            range,
            count,
            heading_count,
        })
    } else {
        None
    };
    let (v, t) = occurrences(text, &window, 65);
    let old_window_count = if t { 66 } else { v.len() };
    let (neighbors, scopes) =
        neighbor_scopes(text, source.source_byte, &window, original_start, &hs);
    if work_cancelled() {
        return None;
    }
    let source_blocks = blocks(text);
    if work_cancelled() {
        return None;
    }
    let block_identity = source_blocks
        .iter()
        .find(|r| r.contains(&source.source_byte))
        .map(|range| {
            let hash = fingerprint(&text[range.clone()]);
            let matches: Vec<_> = source_blocks
                .iter()
                .take_while(|_| !work_cancelled())
                .filter(|r| r.len() == range.len() && fingerprint(&text[(*r).clone()]) == hash)
                .collect();
            BlockIdentity {
                length: range.len(),
                hash,
                target_offset: source.source_byte - range.start,
                old_count: matches.len(),
                old_heading_count: matches
                    .iter()
                    .filter(|r| path_at(&hs, r.start) == heading)
                    .count(),
                scope_counts: scopes
                    .iter()
                    .map(|s| {
                        matches
                            .iter()
                            .filter(|r| {
                                neighbors[s.left].block_range.end <= r.start
                                    && r.end <= neighbors[s.right].block_range.start
                            })
                            .count()
                    })
                    .collect(),
            }
        });
    let bounded_block = source_blocks
        .iter()
        .find(|range| range.contains(&source.source_byte) && range.len() <= 4096)
        .map(|range| {
            let body = &text[range.clone()];
            let mut boundaries: Vec<_> = body.grapheme_indices(true).map(|(b, _)| b).collect();
            boundaries.push(body.len());
            let n = boundaries.len().saturating_sub(1);
            let width = 32.min(n);
            let mut seeds = Vec::new();
            for start in [0, n.saturating_sub(width), n / 4, n / 2, n * 3 / 4] {
                if work_cancelled() {
                    break;
                }
                let start = start.min(n.saturating_sub(width));
                let span = boundaries[start]..boundaries[start + width];
                if span.len() < 24 || seeds.iter().any(|s: &Seed| s.range == span) {
                    continue;
                }
                let (positions, truncated) = occurrences(text, &body[span.clone()], 65);
                if !truncated && positions.len() == 1 {
                    seeds.push(Seed {
                        range: span,
                        count: 1,
                    });
                }
            }
            BoundedBlock {
                source_range: range.clone(),
                text: body.to_owned(),
                seeds,
            }
        });
    Some(Bundle {
        source,
        window,
        target,
        proofs,
        seeds,
        heading,
        global_witnesses: witnesses.into_keys().collect(),
        old_window_count,
        block,
        neighbors,
        scopes,
        block_identity,
        bounded_block,
    })
}
#[derive(Clone, Copy, Debug)]
struct Alignment {
    score: i32,
    mapping: i32,
}
impl Alignment {
    const INVALID: Self = Self {
        score: -1_000_000_000,
        mapping: -2,
    };
    fn cost(mut self, cost: i32) -> Self {
        self.score += cost;
        self
    }
    fn merge(self, other: Self) -> Self {
        if self.score > other.score {
            self
        } else if other.score > self.score {
            other
        } else {
            Self {
                score: self.score,
                mapping: if self.mapping == other.mapping {
                    self.mapping
                } else {
                    -3
                },
            }
        }
    }
    fn target(mut self, at_target: bool, map: i32) -> Self {
        if at_target {
            self.mapping = map
        }
        self
    }
}
fn forced_diff(
    old: &str,
    new: &str,
    target: usize,
    anchored: bool,
) -> Option<(usize, usize, usize)> {
    forced_diff_required(old, new, target, anchored, &[])
}
fn forced_diff_required(
    old: &str,
    new: &str,
    target: usize,
    anchored: bool,
    required: &[(Range<usize>, Range<usize>)],
) -> Option<(usize, usize, usize)> {
    forced_diff_with_fold(old, new, target, anchored, required, true)
        .map(|result| (result.byte, result.hunk, result.score))
}
#[derive(Clone, Debug, Serialize)]
struct DiffResult {
    byte: usize,
    hunk: usize,
    score: usize,
    dynamic_score: i32,
    fixed_match_score: i32,
    fixed_prefix_graphemes: usize,
    fixed_suffix_graphemes: usize,
    dynamic_old_graphemes: usize,
    dynamic_new_graphemes: usize,
}
fn forced_diff_with_fold(
    old: &str,
    new: &str,
    target: usize,
    anchored: bool,
    required: &[(Range<usize>, Range<usize>)],
    fold_edges: bool,
) -> Option<DiffResult> {
    let all_a: Vec<(usize, &str)> = old.grapheme_indices(true).collect();
    let all_b: Vec<(usize, &str)> = new.grapheme_indices(true).collect();
    let mut mandatory = BTreeMap::new();
    for (left, right) in required {
        if old.get(left.clone())? != new.get(right.clone())? {
            return None;
        }
        for (byte, _) in old[left.clone()].grapheme_indices(true) {
            let oi = all_a
                .binary_search_by_key(&(left.start + byte), |v| v.0)
                .ok()?;
            let ni = all_b
                .binary_search_by_key(&(right.start + byte), |v| v.0)
                .ok()?;
            if mandatory
                .insert(oi, ni)
                .is_some_and(|previous| previous != ni)
            {
                return None;
            }
        }
    }
    let all_selected = all_a.iter().position(|v| v.0 == target)?;
    let mut prefix = 0;
    let mut suffix = 0;
    if anchored && fold_edges {
        while prefix < all_selected
            && prefix < all_b.len()
            && mandatory.get(&prefix) == Some(&prefix)
            && all_a[prefix].1 == all_b[prefix].1
        {
            prefix += 1;
        }
        while suffix < all_a.len() - all_selected - 1 && suffix < all_b.len().saturating_sub(prefix)
        {
            let old_at = all_a.len() - suffix - 1;
            let new_at = all_b.len() - suffix - 1;
            if mandatory.get(&old_at) != Some(&new_at) || all_a[old_at].1 != all_b[new_at].1 {
                break;
            }
            suffix += 1;
        }
    }
    let mut dynamic_mandatory = BTreeMap::new();
    for (&oi, &ni) in &mandatory {
        if oi >= prefix && oi < all_a.len() - suffix {
            if ni < prefix || ni >= all_b.len() - suffix {
                return None;
            }
            dynamic_mandatory.insert(oi - prefix, ni - prefix);
        }
    }
    let mandatory = dynamic_mandatory;
    let a = &all_a[prefix..all_a.len() - suffix];
    let b = &all_b[prefix..all_b.len() - suffix];
    let selected = all_selected - prefix;
    let (m, n) = (a.len(), b.len());
    if n > 4096 || m > 1536 {
        return None;
    }
    // Semi-global affine-gap alignment: all old context participates; arbitrary
    // new prefix/suffix is free. Equal best alignments merge mappings rather than
    // choosing first/nearest. Contiguous edit runs carry an explicit opening cost.
    let mut previous = vec![[Alignment::INVALID; 3]; n + 1];
    previous[0][0] = Alignment {
        score: 0,
        mapping: -2,
    };
    for (j, cell) in previous.iter_mut().enumerate().take(n + 1).skip(1) {
        if anchored {
            cell[2] = Alignment {
                score: -8 - (j as i32 - 1),
                mapping: -2,
            };
        } else {
            cell[0] = Alignment {
                score: 0,
                mapping: -2,
            };
        }
    }
    for i in 1..=m {
        if work_cancelled() {
            return None;
        }
        let mut current = vec![[Alignment::INVALID; 3]; n + 1];
        current[0][1] = previous[0][0]
            .cost(-8)
            .merge(previous[0][1].cost(-1))
            .merge(previous[0][2].cost(-8))
            .target(i - 1 == selected, -1);
        if mandatory.contains_key(&(i - 1)) {
            current[0][1] = Alignment::INVALID;
        }
        for j in 1..=n {
            let equal = a[i - 1].1 == b[j - 1].1;
            current[j][0] = previous[j - 1]
                .iter()
                .copied()
                .reduce(Alignment::merge)
                .unwrap()
                .cost(if equal { 4 } else { -5 })
                .target(i - 1 == selected, if equal { (j - 1) as i32 } else { -1 });
            current[j][1] = previous[j][0]
                .cost(-8)
                .merge(previous[j][1].cost(-1))
                .merge(previous[j][2].cost(-8))
                .target(i - 1 == selected, -1);
            if let Some(required_j) = mandatory.get(&(i - 1)) {
                if *required_j != j - 1 {
                    current[j][0] = Alignment::INVALID;
                }
                current[j][1] = Alignment::INVALID;
            }
            current[j][2] = current[j - 1][0]
                .cost(-8)
                .merge(current[j - 1][1].cost(-8))
                .merge(current[j - 1][2].cost(-1));
        }
        previous = current;
    }
    let mut best = if anchored {
        previous[n].iter().copied().reduce(Alignment::merge)?
    } else {
        previous
            .iter()
            .flat_map(|cell| cell.iter())
            .copied()
            .reduce(Alignment::merge)?
    };
    let dynamic_score = best.score;
    // Removed positions are mandatory equal diagonals, each worth exactly 4.
    // Every valid path gets this identical constant, preserving the complete
    // path score ordering, runner-up margin and target-mapping ambiguity.
    let fixed_match_score = ((prefix + suffix) * 4) as i32;
    best.score += fixed_match_score;
    if best.mapping < 0 || (!anchored && best.score < m as i32) {
        return None;
    }
    let j = best.mapping as usize + prefix;
    let i = all_selected;
    let mut left = 0;
    let mut right = 1;
    while i > left && j > left && all_a[i - left - 1].1 == all_b[j - left - 1].1 {
        left += 1
    }
    while i + right < all_a.len()
        && j + right < all_b.len()
        && all_a[i + right].1 == all_b[j + right].1
    {
        right += 1
    }
    if anchored {
        if left + right < 12 && old != new {
            return None;
        }
    } else if left + right < 12 || (left < 2 && i > 0) || (right < 3 && i + 1 < all_a.len()) {
        return None;
    }
    Some(DiffResult {
        byte: all_b[j].0,
        hunk: left + right,
        score: best.score.max(0) as usize,
        dynamic_score,
        fixed_match_score,
        fixed_prefix_graphemes: prefix,
        fixed_suffix_graphemes: suffix,
        dynamic_old_graphemes: m,
        dynamic_new_graphemes: n,
    })
}

fn resolve_bundle(text: &str, anchor: &Bundle) -> Decision {
    let same = resolve_unchanged(text, &anchor.source);
    if same.exact {
        return Decision {
            exact: true,
            source_byte: same.source_byte,
            reason: "unchanged".into(),
            candidates: vec![same.source_byte],
            evidence: serde_json::json!({}),
        };
    }
    let fallback = |reason: &str, candidates: Vec<usize>| Decision {
        exact: false,
        source_byte: anchor.source.source_byte.min(text.len()),
        reason: reason.into(),
        candidates,
        evidence: serde_json::json!({}),
    };
    if text.is_empty() || anchor.source.grapheme.is_empty() {
        return fallback("empty/invalid", vec![]);
    }
    let witnesses: BTreeSet<u64> = anchor.global_witnesses.iter().copied().collect();
    let mut shared = BTreeSet::new();
    shingles(text, |hash, _| {
        if witnesses.contains(&hash) {
            shared.insert(hash);
        }
    });
    if witnesses.len() < 2 || shared.len() < 2 {
        return fallback("insufficient nonlocal manuscript evidence", vec![]);
    }
    if work_cancelled() {
        return fallback("cancelled", vec![]);
    }
    let hs = headings(text);
    if work_cancelled() {
        return fallback("cancelled", vec![]);
    }
    let source_blocks = blocks(text);
    if work_cancelled() {
        return fallback("cancelled", vec![]);
    }
    let mut strong: BTreeMap<usize, usize> = BTreeMap::new();
    let mut any_truncated = false;
    if let Some(identity) = &anchor.block_identity {
        let candidates: Vec<Range<usize>> = blocks(text)
            .into_iter()
            .filter(|r| {
                r.len() == identity.length && fingerprint(&text[r.clone()]) == identity.hash
            })
            .collect();
        let mut accepted = BTreeSet::new();
        if identity.length >= 64 && identity.old_count == 1 && candidates.len() == 1 {
            accepted.insert(candidates[0].start + identity.target_offset);
        }
        if identity.length >= 64 && identity.old_heading_count == 1 {
            let same: Vec<_> = candidates
                .iter()
                .filter(|r| path_at(&hs, r.start) == anchor.heading)
                .collect();
            if same.len() == 1 {
                accepted.insert(same[0].start + identity.target_offset);
            }
        }
        if identity.length < 64 && identity.old_count == 1 && candidates.len() == 1 {
            let original_start = anchor.source.source_byte - identity.target_offset;
            let original_end = original_start + identity.length;
            for neighbor in &anchor.neighbors {
                if neighbor.count != 1 {
                    continue;
                }
                if work_cancelled() {
                    return fallback("cancelled", vec![]);
                }
                let before = neighbor.block_range.end <= original_start
                    && original_start - neighbor.block_range.end <= 8;
                let after = neighbor.block_range.start >= original_end
                    && neighbor.block_range.start - original_end <= 8;
                if !before && !after {
                    continue;
                }
                let (positions, truncated) =
                    neighbor_occurrences(text, neighbor, &source_blocks, &hs);
                if truncated || positions.len() != 1 {
                    continue;
                }
                if let Some(block) = source_blocks.iter().find(|b| {
                    b.start <= positions[0] && positions[0] + neighbor.text.len() <= b.end
                }) {
                    let candidate = &candidates[0];
                    if before
                        && block.end <= candidate.start
                        && text[block.end..candidate.start].trim().is_empty()
                        || after
                            && candidate.end <= block.start
                            && text[candidate.end..block.start].trim().is_empty()
                    {
                        accepted.insert(candidate.start + identity.target_offset);
                    }
                }
            }
        }
        for (i, scope) in anchor.scopes.iter().enumerate() {
            if work_cancelled() {
                return fallback("cancelled", vec![]);
            }
            if identity.scope_counts[i] != 1 {
                continue;
            }
            let left = &anchor.neighbors[scope.left];
            let right = &anchor.neighbors[scope.right];
            if left.count != 1 || right.count != 1 {
                continue;
            }
            let (l, lt) = neighbor_occurrences(text, left, &source_blocks, &hs);
            let (r, rt) = neighbor_occurrences(text, right, &source_blocks, &hs);
            if lt || rt || l.len() != 1 || r.len() != 1 {
                continue;
            }
            let Some(left_block) = source_blocks
                .iter()
                .find(|b| b.start <= l[0] && l[0] + left.text.len() <= b.end)
            else {
                continue;
            };
            let Some(right_block) = source_blocks
                .iter()
                .find(|b| b.start <= r[0] && r[0] + right.text.len() <= b.end)
            else {
                continue;
            };
            let same: Vec<_> = candidates
                .iter()
                .filter(|c| left_block.end <= c.start && c.end <= right_block.start)
                .collect();
            if same.len() == 1 {
                accepted.insert(same[0].start + identity.target_offset);
            }
        }
        if accepted.len() == 1 {
            let byte = *accepted.first().unwrap();
            let range = grapheme_range(text, byte);
            if range.start == byte && text[range] == anchor.source.grapheme {
                return Decision {
                    exact: true,
                    source_byte: byte,
                    reason: "complete unchanged source block identity and verified scope".into(),
                    candidates: accepted.into_iter().collect(),
                    evidence: serde_json::json!({"complete_source_bytes":identity.length,"source_offset_within_identical_block":identity.target_offset,"old_global_count":identity.old_count}),
                };
            }
        }
        if accepted.len() > 1 {
            return fallback(
                "complete block scopes disagree",
                accepted.into_iter().collect(),
            );
        }
    }
    for proof in &anchor.proofs {
        if work_cancelled() {
            return fallback("cancelled", vec![]);
        }
        let (positions, truncated) = occurrences(text, &anchor.window[proof.range.clone()], 65);
        any_truncated |= truncated;
        if truncated {
            continue;
        }
        let candidates: Vec<usize> = positions
            .into_iter()
            .map(|p| p + anchor.target.start - proof.range.start)
            .collect();
        let accepted: Vec<usize> = if proof.count == 1 {
            candidates
        } else if !anchor.heading.is_empty() && proof.heading_count == 1 {
            candidates
                .into_iter()
                .filter(|p| path_at(&hs, *p) == anchor.heading)
                .collect()
        } else {
            vec![]
        };
        if accepted.len() == 1 {
            strong
                .entry(accepted[0])
                .and_modify(|score| *score = (*score).max(proof.range.len()))
                .or_insert(proof.range.len());
        }
    }
    if let Some(max) = strong.values().max().copied() {
        let top: Vec<usize> = strong
            .iter()
            .filter_map(|(p, s)| (*s == max).then_some(*p))
            .collect();
        if top.len() == 1 && max == anchor.window.len() {
            let byte = top[0];
            let old_inside = anchor.block_identity.as_ref().is_some_and(|id| {
                id.target_offset >= anchor.target.start
                    && id.target_offset - anchor.target.start + anchor.window.len() <= id.length
            });
            let new_inside = source_blocks.iter().any(|b| {
                b.start <= byte.saturating_sub(anchor.target.start)
                    && byte - anchor.target.start + anchor.window.len() <= b.end
            });
            if !old_inside || !new_inside {
                return fallback(
                    "cross-block context cannot prove the original block survived",
                    top,
                );
            }
            let range = grapheme_range(text, byte);
            if range.start == byte && text[range.clone()] == anchor.source.grapheme {
                return Decision {
                    exact: true,
                    source_byte: byte,
                    reason: "globally verified unique context".into(),
                    candidates: top,
                    evidence: serde_json::json!({"strongest_unchanged_bytes":max,"nonlocal_shared":shared.len()}),
                };
            }
        }
    }

    let mut scoped = BTreeSet::new();
    let mut scope_evidence = vec![];
    for scope in &anchor.scopes {
        if work_cancelled() {
            return fallback("cancelled", vec![]);
        }
        if scope.old_count != 1 {
            continue;
        }
        let left = &anchor.neighbors[scope.left];
        let right = &anchor.neighbors[scope.right];
        if left.count != 1 || right.count != 1 {
            continue;
        }
        let (l, lt) = neighbor_occurrences(text, left, &source_blocks, &hs);
        let (r, rt) = neighbor_occurrences(text, right, &source_blocks, &hs);
        if lt || rt || l.len() != 1 || r.len() != 1 {
            continue;
        }
        let Some(left_block) = source_blocks
            .iter()
            .find(|b| b.start <= l[0] && l[0] + left.text.len() <= b.end)
        else {
            continue;
        };
        let Some(right_block) = source_blocks
            .iter()
            .find(|b| b.start <= r[0] && r[0] + right.text.len() <= b.end)
        else {
            continue;
        };
        let start = left_block.end;
        let end = right_block.start;
        if start > end {
            continue;
        }
        let (matches, truncated) = occurrences(text, &anchor.window, 65);
        let matches: Vec<_> = matches
            .into_iter()
            .filter(|at| {
                let byte = *at + anchor.target.start;
                (start..end).contains(&byte)
                    && source_blocks
                        .iter()
                        .any(|b| b.start <= *at && *at + anchor.window.len() <= b.end)
            })
            .collect();
        let old_inside = anchor.block_identity.as_ref().is_some_and(|id| {
            id.target_offset >= anchor.target.start
                && id.target_offset - anchor.target.start + anchor.window.len() <= id.length
        });
        if old_inside && !truncated && matches.len() == 1 {
            let byte = matches[0] + anchor.target.start;
            scoped.insert(byte);
            scope_evidence.push(serde_json::json!({"left":l[0],"right":r[0],"target":byte,"old_count":scope.old_count}));
        }
    }
    if scoped.len() == 1 {
        return Decision {
            exact: true,
            source_byte: *scoped.first().unwrap(),
            reason: "unchanged context unique within verified neighboring-block order".into(),
            candidates: scoped.into_iter().collect(),
            evidence: serde_json::json!({"neighbor_scopes":scope_evidence}),
        };
    }
    if scoped.len() > 1 {
        return fallback(
            "verified neighboring scopes disagree",
            scoped.into_iter().collect(),
        );
    }
    if let Some(decision) = bounded_block_diff(text, anchor, &source_blocks, &hs) {
        return decision;
    }

    if let Some(block) = &anchor.block {
        let (v, t) = occurrences(text, &anchor.window[block.range.clone()], 65);
        if !t {
            let candidates: Vec<usize> = v
                .into_iter()
                .map(|p| p + anchor.target.start - block.range.start)
                .filter(|p| {
                    block.count == 1
                        || (!anchor.heading.is_empty()
                            && block.heading_count == 1
                            && path_at(&hs, *p) == anchor.heading)
                })
                .collect();
            if candidates.len() == 1 {
                return Decision {
                    exact: true,
                    source_byte: candidates[0],
                    reason: "globally unique unchanged source paragraph".into(),
                    candidates,
                    evidence: serde_json::json!({"old_count":block.count,"old_heading_count":block.heading_count}),
                };
            }
        }
    }
    // A truncated short proof is unusable, but cannot invalidate a separate,
    // fully enumerated unique witness corridor. No prefix score becomes exact.
    if anchor.old_window_count != 1 {
        return fallback("old context ambiguous", strong.into_keys().collect());
    }

    // Match two immutable unique source witnesses outside the selected grapheme.
    // Their old order defines a verifiable diff corridor; new distance is never
    // used to pick among repeated candidates. Deleted whole paragraph cannot be
    // mistaken for its similar successor by shifting all matching IDs.
    let mut left = vec![];
    let mut right = vec![];
    for seed in &anchor.seeds {
        if work_cancelled() {
            return fallback("cancelled", vec![]);
        }
        if seed.count != 1 {
            continue;
        }
        let (matches, truncated) = occurrences(text, &anchor.window[seed.range.clone()], 65);
        if truncated {
            continue;
        }
        if matches.len() != 1 {
            continue;
        }
        if seed.range.end <= anchor.target.start {
            left.push((seed.range.end, matches[0] + seed.range.len()));
        }
        if seed.range.start >= anchor.target.end {
            right.push((seed.range.start, matches[0]));
        }
    }
    left.sort_by_key(|p| std::cmp::Reverse(p.0));
    right.sort_by_key(|p| p.0);
    if let (Some(&(old_left, new_left)), Some(&(old_right, new_right))) =
        (left.first(), right.first())
        && new_left <= new_right
        && old_left <= anchor.target.start
        && old_right >= anchor.target.end
    {
        let old = &anchor.window[old_left..old_right];
        let new = &text[new_left..new_right];
        if let Some((relative, hunk, score)) =
            forced_diff(old, new, anchor.target.start - old_left, true)
        {
            let byte = new_left + relative;
            return Decision {
                exact: true,
                source_byte: byte,
                reason: "unique witnesses constrain all best affine-gap mappings".into(),
                candidates: vec![byte],
                evidence: serde_json::json!({"old_corridor":[old_left,old_right],"new_corridor":[new_left,new_right],"hunk":hunk,"score":score}),
            };
        }
        return fallback(
            "target deleted or ambiguous within immutable witness corridor",
            vec![],
        );
    }
    // Unconstrained best alignment can shift a deleted paragraph onto a similar
    // surviving neighbor. Without immutable witnesses there is no exact claim.
    fallback(
        if any_truncated {
            "truncated weak evidence; no immutable unique corridor"
        } else {
            "no immutable witness corridor establishes a unique surviving target"
        },
        strong.into_keys().collect(),
    )
}

// Reconstruct only literal bytes actually retained by the old proof. A
// neighbor gap belongs to its immediately adjacent source node toward the
// selected node. Missing links or contradictory overlapping fragments fail.
fn saved_source_corridor(anchor: &Bundle, range: Range<usize>) -> Option<String> {
    if range.start > range.end || range.len() > 8192 {
        return None;
    }
    let old = anchor.bounded_block.as_ref()?;
    let mut nodes = vec![old.source_range.clone()];
    nodes.extend(anchor.neighbors.iter().map(|n| n.block_range.clone()));
    nodes.sort_by_key(|r| r.start);
    nodes.dedup();
    let origin = anchor
        .source
        .source_byte
        .checked_sub(anchor.source.before.len())?;
    let mut pieces = vec![
        (old.source_range.clone(), old.text.as_str()),
        (origin..origin + anchor.window.len(), anchor.window.as_str()),
    ];
    for neighbor in &anchor.neighbors {
        pieces.push((neighbor.source_range.clone(), neighbor.text.as_str()));
        if let Some(complete) = neighbor.complete.as_deref() {
            pieces.push((neighbor.block_range.clone(), complete));
        }
        if let Some(gap) = neighbor.adjacent_gap.as_deref() {
            let bridge = if neighbor.block_range.end <= old.source_range.start {
                nodes
                    .iter()
                    .find(|r| r.start >= neighbor.block_range.end)
                    .map(|r| neighbor.block_range.end..r.start)
            } else if neighbor.block_range.start >= old.source_range.end {
                nodes
                    .iter()
                    .rev()
                    .find(|r| r.end <= neighbor.block_range.start)
                    .map(|r| r.end..neighbor.block_range.start)
            } else {
                None
            };
            if let Some(bridge) = bridge.filter(|r| r.len() == gap.len()) {
                pieces.push((bridge, gap));
            }
        }
    }
    let mut bytes = vec![None; range.len()];
    for (source, literal) in pieces {
        if source.len() != literal.len() {
            return None;
        }
        let start = source.start.max(range.start);
        let end = source.end.min(range.end);
        if start >= end {
            continue;
        }
        for at in start..end {
            let value = literal.as_bytes()[at - source.start];
            let cell = &mut bytes[at - range.start];
            if cell.is_some_and(|previous| previous != value) {
                return None;
            }
            *cell = Some(value);
        }
    }
    String::from_utf8(bytes.into_iter().collect::<Option<Vec<_>>>()?).ok()
}
struct DiffCorridor {
    source_range: Range<usize>,
    old_text: String,
    target: usize,
    internal_shift: usize,
    external: Vec<(Range<usize>, Range<usize>)>,
}
fn neighbor_matches(
    text: &str,
    neighbor: &Neighbor,
    source_blocks: &[Range<usize>],
    hs: &[(usize, Vec<String>)],
) -> Vec<(Range<usize>, Range<usize>)> {
    if neighbor.text.len() < 24 {
        let (positions, _) = neighbor_occurrences(text, neighbor, source_blocks, hs);
        return positions
            .into_iter()
            .map(|at| (neighbor.block_range.clone(), at..at + neighbor.text.len()))
            .collect();
    }
    let mut result = Vec::new();
    for piece in std::iter::once(0..neighbor.text.len())
        .filter(|_| neighbor.count == 1)
        .chain(neighbor.unique_fragments.iter().map(|s| s.range.clone()))
    {
        let (positions, truncated) = occurrences(text, &neighbor.text[piece.clone()], 65);
        if !truncated && positions.len() == 1 {
            result.push((
                neighbor.source_range.start + piece.start..neighbor.source_range.start + piece.end,
                positions[0]..positions[0] + piece.len(),
            ));
            if piece.start == 0 && piece.end == neighbor.text.len() {
                // The complete immutable fragment subsumes its smaller
                // witnesses; no location score or nearest-copy choice occurs.
                break;
            }
        }
    }
    if let Some(complete) = &neighbor.complete {
        for seed in &neighbor.complete_witnesses {
            let (positions, truncated) = occurrences(text, &complete[seed.range.clone()], 65);
            if seed.count == 1 && !truncated && positions.len() == 1 {
                result.push((
                    neighbor.block_range.start + seed.range.start
                        ..neighbor.block_range.start + seed.range.end,
                    positions[0]..positions[0] + seed.range.len(),
                ));
            }
        }
    }
    result
}
fn bounded_block_diff(
    text: &str,
    anchor: &Bundle,
    source_blocks: &[Range<usize>],
    hs: &[(usize, Vec<String>)],
) -> Option<Decision> {
    let old = anchor.bounded_block.as_ref()?;
    let target = anchor
        .source
        .source_byte
        .checked_sub(old.source_range.start)?;
    let mut intervals = BTreeSet::new();
    let mut witnesses = Vec::new();
    for seed in &old.seeds {
        if work_cancelled() {
            return None;
        }
        if seed.count != 1 {
            continue;
        }
        let (positions, truncated) = occurrences(text, &old.text[seed.range.clone()], 65);
        if !truncated && positions.len() == 1 {
            let at = positions[0];
            if let Some(block) = source_blocks
                .iter()
                .find(|b| b.start <= at && at + seed.range.len() <= b.end)
            {
                intervals.insert((block.start, block.end));
                witnesses.push((seed.range.clone(), at..at + seed.range.len()));
            }
        }
    }
    // Include a retained neighbor fragment in the OLD diff input as well as
    // hard-constraining its NEW location. Subtracting its old offset from a new
    // location would be invalid when a split/insert changed the prefix itself.
    // Literal bytes between the old node and witness come from saved context.
    let mut extended = Vec::new();
    let origin = anchor
        .source
        .source_byte
        .checked_sub(anchor.source.before.len())?;
    if !witnesses.is_empty() {
        for neighbor in &anchor.neighbors {
            if neighbor.block_range.start < old.source_range.end
                || neighbor.block_range.start - old.source_range.end > 8
            {
                continue;
            }
            for piece in std::iter::once(0..neighbor.text.len())
                .filter(|_| neighbor.count == 1)
                .chain(neighbor.unique_fragments.iter().map(|s| s.range.clone()))
            {
                let (positions, truncated) = if neighbor.text.len() < 24 {
                    if piece != (0..neighbor.text.len()) {
                        continue;
                    }
                    neighbor_occurrences(text, neighbor, source_blocks, hs)
                } else {
                    occurrences(text, &neighbor.text[piece.clone()], 65)
                };
                if truncated || positions.len() != 1 {
                    continue;
                }
                let old_end = neighbor.source_range.start + piece.end;
                let Some(suffix_start) = old.source_range.end.checked_sub(origin) else {
                    continue;
                };
                let Some(suffix_end) = old_end.checked_sub(origin) else {
                    continue;
                };
                let Some(suffix) = anchor.window.get(suffix_start..suffix_end) else {
                    continue;
                };
                let old_input = old.text.clone() + suffix;
                let old_witness = neighbor.source_range.start + piece.start - old.source_range.start
                    ..old_input.len();
                let new_witness = positions[0]..positions[0] + piece.len();
                for &(start, _) in &intervals {
                    if start < new_witness.start {
                        extended.push(DiffCorridor {
                            source_range: start..new_witness.end,
                            old_text: old_input.clone(),
                            target,
                            internal_shift: 0,
                            external: vec![(old_witness.clone(), new_witness.clone())],
                        });
                    }
                }
            }
        }
    }
    // Without a surviving unique internal witness, a similar successor must
    // not be mistaken for a deleted original block.
    let mut structural_scopes = Vec::new();
    for (scope_index, scope) in anchor.scopes.iter().enumerate() {
        if work_cancelled() {
            return None;
        }
        let left = &anchor.neighbors[scope.left];
        let right = &anchor.neighbors[scope.right];
        if anchor
            .block_identity
            .as_ref()
            .is_none_or(|id| id.scope_counts.get(scope_index) != Some(&1))
            || left.block_range.end > old.source_range.start
            || old.source_range.end > right.block_range.start
        {
            continue;
        }
        // The complete old node is unique within this verified scope. Its
        // shorter context window may repeat arbitrarily; that does not discard
        // the independent complete-node all-best mapping proof.
        for (old_left, new_left) in neighbor_matches(text, left, source_blocks, hs) {
            for (old_right, new_right) in neighbor_matches(text, right, source_blocks, hs) {
                if new_left.end > new_right.start
                    || old_left.end > old.source_range.start
                    || old_right.start < old.source_range.end
                {
                    continue;
                }
                let old_range = old_left.start..old_right.end;
                let Some(old_input) = saved_source_corridor(anchor, old_range.clone()) else {
                    continue;
                };
                let shift = old.source_range.start - old_range.start;
                let right_start = old_right.start - old_range.start;
                let old_end = old_input.len();
                extended.push(DiffCorridor {
                    source_range: new_left.start..new_right.end,
                    old_text: old_input,
                    target: target + shift,
                    internal_shift: shift,
                    external: vec![
                        (0..old_left.len(), new_left.clone()),
                        (right_start..old_end, new_right.clone()),
                    ],
                });
                structural_scopes.push(serde_json::json!({"old_neighbor_indexes":[scope.left,scope.right],"new_outer_fragments":[new_left,new_right],"old_complete_block_count_within_scope":1,"old_literal_corridor":old_range}));
            }
        }
        // The old narrower interval-only fallback remains direct-neighbor
        // only; distant nodes require the complete literal chain above.
        if scope.old_count != 1
            || old.source_range.start - left.block_range.end > 8
            || right.block_range.start - old.source_range.end > 8
        {
            continue;
        }
        if left.count != 1 || right.count != 1 {
            continue;
        }
        let (l, lt) = neighbor_occurrences(text, left, source_blocks, hs);
        let (r, rt) = neighbor_occurrences(text, right, source_blocks, hs);
        if lt || rt || l.len() != 1 || r.len() != 1 {
            continue;
        }
        let Some(lb) = source_blocks
            .iter()
            .find(|b| b.start <= l[0] && l[0] + left.text.len() <= b.end)
        else {
            continue;
        };
        let Some(rb) = source_blocks
            .iter()
            .find(|b| b.start <= r[0] && r[0] + right.text.len() <= b.end)
        else {
            continue;
        };
        if lb.end < rb.start
            && anchor
                .block_identity
                .as_ref()
                .is_some_and(|id| id.scope_counts.get(scope_index) == Some(&1))
        {
            let inside: Vec<_> = source_blocks
                .iter()
                .filter(|b| lb.end <= b.start && b.end <= rb.start)
                .collect();
            if inside.len() == 1 {
                let block = inside[0];
                intervals.insert((block.start, block.end));
                structural_scopes.push(serde_json::json!({"old_neighbor_indexes":[scope.left,scope.right],"new_neighbor_blocks":[lb,rb],"single_new_source_block":block,"old_complete_block_count_within_scope":1}));
            }
        }
        if lb.end < rb.start
            && witnesses
                .iter()
                .any(|(_, w)| lb.end <= w.start && w.end <= rb.start)
        {
            intervals.insert((lb.end, rb.start));
        }
    }
    if witnesses.is_empty() && structural_scopes.is_empty() {
        return None;
    }
    let mut accepted = BTreeSet::new();
    let mut evidence = Vec::new();
    let contexts = if extended.is_empty() {
        intervals
            .into_iter()
            .map(|(start, end)| DiffCorridor {
                source_range: start..end,
                old_text: old.text.clone(),
                target,
                internal_shift: 0,
                external: vec![],
            })
            .collect()
    } else {
        extended
    };
    for context in contexts {
        let DiffCorridor {
            source_range,
            old_text: old_input,
            target,
            internal_shift,
            external,
        } = context;
        let (start, end) = (source_range.start, source_range.end);
        if work_cancelled() {
            return None;
        }
        if end - start > 8192 {
            continue;
        }
        if witnesses
            .iter()
            .any(|(_, new)| new.start < start || new.end > end)
        {
            continue;
        }
        let mut required: Vec<_> = witnesses
            .iter()
            .map(|(old, new)| {
                (
                    old.start + internal_shift..old.end + internal_shift,
                    new.start - start..new.end - start,
                )
            })
            .collect();
        for (old, new) in external {
            required.push((old, new.start - start..new.end - start));
        }
        if let Some(alignment) =
            forced_diff_with_fold(&old_input, &text[start..end], target, true, &required, true)
        {
            let (relative, hunk, score) = (alignment.byte, alignment.hunk, alignment.score);
            if witnesses.is_empty() && score < old.text.graphemes(true).count() * 2 {
                continue;
            }
            let byte = start + relative;
            let range = grapheme_range(text, byte);
            if range.start == byte && text[range] == anchor.source.grapheme {
                accepted.insert(byte);
                evidence.push(serde_json::json!({"new_source_range":[start,end],"old_input_bytes":old_input.len(),"required_source_correspondences":required,"target":byte,"unchanged_target_hunk":hunk,"score":score,"fixed_boundary_alignment":alignment}));
            }
        }
    }
    if accepted.is_empty() {
        return None;
    }
    let exact = accepted.len() == 1;
    Some(Decision {
        exact,
        source_byte: *accepted.first()?,
        reason: if exact {
            "unique surviving block witnesses constrain all-best bounded block diff"
        } else {
            "bounded block diff corridors disagree"
        }
        .into(),
        candidates: accepted.into_iter().collect(),
        evidence: serde_json::json!({"old_source_range":old.source_range,"globally_unique_old_new_witnesses":witnesses,"verified_unique_structural_scopes":structural_scopes,"bounded_diff_corridors":evidence,"all_best_diff_mappings_agree":exact}),
    })
}

impl Anchor {
    pub fn has_relocation_proof(&self) -> bool {
        self.proof.is_some()
    }
    pub fn estimated_proof_bytes(&self) -> usize {
        self.proof.as_ref().map_or(0, |p| {
            512 + p.proofs.len() * 64
                + p.seeds.len() * 48
                + p.heading.iter().map(String::len).sum::<usize>()
                + p.global_witnesses.len() * 8
                + p.neighbors
                    .iter()
                    .map(|n| {
                        128 + n.text.len()
                            + n.unique_fragments.len() * 48
                            + n.complete.as_ref().map_or(0, String::len)
                            + n.complete_witnesses.len() * 48
                    })
                    .sum::<usize>()
                + p.scopes.len() * 32
                + p.block_identity
                    .as_ref()
                    .map_or(0, |b| 64 + b.scope_counts.len() * 8)
                + p.bounded_block
                    .as_ref()
                    .map_or(0, |b| 128 + b.text.len() + b.seeds.len() * 48)
        })
    }
    pub fn valid(&self) -> bool {
        if self.source_len > MAX_SOURCE_BYTES
            || self.before.len() > 4096
            || self.after.len() > 4096
            || self.grapheme.len() > 1024
            || self.grapheme.is_empty()
            || self.grapheme_range.start > self.grapheme_range.end
            || self.grapheme_range.end > self.source_len
            || self.source_byte != self.grapheme_range.start
            || self.grapheme_range.len() != self.grapheme.len()
            || self.source_byte < self.before.len()
            || self
                .grapheme_range
                .end
                .checked_add(self.after.len())
                .is_none_or(|end| end > self.source_len)
        {
            return false;
        }
        let window = format!("{}{}{}", self.before, self.grapheme, self.after);
        let target = self.before.len()..self.before.len() + self.grapheme.len();
        if grapheme_range(&window, target.start) != target {
            return false;
        }
        let Some(p) = &self.proof else { return true };
        if p.proofs.len() > 16
            || p.seeds.len() > 24
            || p.heading.len() > 6
            || p.heading
                .iter()
                .any(|h| h.len() != 16 || !h.bytes().all(|c| c.is_ascii_hexdigit()))
            || p.global_witnesses.len() > 16
            || p.old_window_count > 66
            || p.neighbors.len() > 4
            || p.scopes.len() > 4
        {
            return false;
        }
        let range = |r: &Range<usize>| {
            r.start <= r.end
                && r.end <= window.len()
                && window.is_char_boundary(r.start)
                && window.is_char_boundary(r.end)
        };
        let proof = |v: &Proof| {
            range(&v.range)
                && v.range.start <= target.start
                && v.range.end >= target.end
                && v.count <= 66
                && v.heading_count <= 66
        };
        if p.proofs.iter().any(|v| !proof(v))
            || p.block.as_ref().is_some_and(|v| !proof(v))
            || p.seeds.iter().any(|v| !range(&v.range) || v.count > 66)
        {
            return false;
        }
        if p.neighbors.iter().any(|n| {
            n.text.is_empty()
                || n.text.len() > 256
                || (n.text.len() < 24
                    && (n.source_range != n.block_range || !complete_short_heading(&n.text)))
                || n.count == 0
                || n.count > 66
                || (n.count != 1 && (n.complete.is_none() || n.complete_witnesses.is_empty()))
                || n.block_range.start > n.block_range.end
                || n.block_range.end > self.source_len
                || n.source_range.start > n.source_range.end
                || n.source_range.start < n.block_range.start
                || n.source_range.end > n.block_range.end
                || n.source_range.len() != n.text.len()
                || n.complete.as_ref().is_some_and(|complete| {
                    complete.len() > 4096
                        || complete.len() != n.block_range.len()
                        || complete.get(
                            n.source_range.start - n.block_range.start
                                ..n.source_range.end - n.block_range.start,
                        ) != Some(n.text.as_str())
                })
                || n.unique_fragments.len() > 4
                || n.complete_witnesses.len() > 2
                || n.complete_witnesses.iter().any(|seed| {
                    seed.count != 1
                        || n.complete.as_ref().is_none_or(|complete| {
                            seed.range.start > seed.range.end
                                || seed.range.end > complete.len()
                                || !complete.is_char_boundary(seed.range.start)
                                || !complete.is_char_boundary(seed.range.end)
                                || complete[seed.range.clone()].graphemes(true).count() < 8
                                || grapheme_range(complete, seed.range.start).start
                                    != seed.range.start
                                || (seed.range.end != complete.len()
                                    && grapheme_range(complete, seed.range.end).start
                                        != seed.range.end)
                        })
                })
                || n.adjacent_gap
                    .as_ref()
                    .is_some_and(|s| s.len() > 8 || !s.trim().is_empty())
                || n.unique_fragments.iter().any(|p| {
                    p.count != 1
                        || p.range.start > p.range.end
                        || p.range.end > n.text.len()
                        || !n.text.is_char_boundary(p.range.start)
                        || !n.text.is_char_boundary(p.range.end)
                        || n.text[p.range.clone()].graphemes(true).count() < 8
                        || grapheme_range(&n.text, p.range.start).start != p.range.start
                        || (p.range.end != n.text.len()
                            && grapheme_range(&n.text, p.range.end).start != p.range.end)
                })
        }) {
            return false;
        }
        if p.scopes.iter().any(|s| {
            s.left >= p.neighbors.len()
                || s.right >= p.neighbors.len()
                || s.left == s.right
                || s.old_count > 66
                || p.neighbors[s.left].block_range.end > self.source_byte
                || p.neighbors[s.right].block_range.start < self.grapheme_range.end
        }) {
            return false;
        }
        if p.block_identity.as_ref().is_some_and(|b| {
            b.length > self.source_len
                || b.target_offset >= b.length
                || b.target_offset > self.source_byte
                || b.target_offset
                    .checked_add(self.grapheme.len())
                    .is_none_or(|n| n > b.length)
                || (self.source_byte - b.target_offset)
                    .checked_add(b.length)
                    .is_none_or(|n| n > self.source_len)
                || b.old_count > 66
                || b.old_heading_count > 66
                || b.scope_counts.len() != p.scopes.len()
                || b.scope_counts.iter().any(|n| *n > 66)
        }) {
            return false;
        }
        if p.bounded_block.as_ref().is_some_and(|b| {
            b.text.len() > 4096
                || b.source_range.start > b.source_range.end
                || b.source_range.end > self.source_len
                || b.source_range.len() != b.text.len()
                || b.source_range.start > self.source_byte
                || b.source_range.end < self.grapheme_range.end
                || b.seeds.len() > 5
                || b.seeds.iter().any(|s| {
                    s.count != 1
                        || s.range.start > s.range.end
                        || s.range.end > b.text.len()
                        || !b.text.is_char_boundary(s.range.start)
                        || !b.text.is_char_boundary(s.range.end)
                })
                || p.block_identity.as_ref().is_none_or(|id| {
                    id.length != b.text.len()
                        || id.target_offset != self.source_byte - b.source_range.start
                        || id.hash != fingerprint(&b.text)
                })
        }) {
            return false;
        }
        serde_json::to_vec(p).is_ok_and(|bytes| bytes.len() <= 8192)
    }
}

/// Background/pure capture. GUI hot paths use capture_with_fingerprint instead.
pub fn enrich_with_fingerprint(text: &str, byte: usize, source_fingerprint: u64) -> Anchor {
    let base = capture_with_fingerprint(text, byte, source_fingerprint);
    if work_cancelled() || base.grapheme.is_empty() || text.len() > MAX_SOURCE_BYTES {
        return base;
    }
    let block_count = blocks(text).len();
    if work_cancelled() || block_count > MAX_BLOCKS {
        return base;
    }
    let Some(b) = capture_bundle(text, byte, source_fingerprint) else {
        return base;
    };
    let mut source = b.source;
    source.proof = Some(Box::new(AnchorProof {
        proofs: b.proofs,
        seeds: b.seeds,
        heading: b.heading,
        global_witnesses: b.global_witnesses,
        old_window_count: b.old_window_count,
        block: b.block,
        neighbors: b.neighbors,
        scopes: b.scopes,
        block_identity: b.block_identity.map(|mut v| {
            v.old_count = v.old_count.min(66);
            v.old_heading_count = v.old_heading_count.min(66);
            for c in &mut v.scope_counts {
                *c = (*c).min(66)
            }
            v
        }),
        bounded_block: b.bounded_block,
    }));
    while !source.valid()
        && source.proof.as_ref().is_some_and(|p| {
            p.neighbors
                .iter()
                .any(|n| n.count == 1 && n.complete.is_some())
        })
    {
        if let Some(proof) = &mut source.proof
            && let Some(neighbor) = proof
                .neighbors
                .iter_mut()
                .rev()
                .find(|n| n.count == 1 && n.complete.is_some())
        {
            neighbor.complete = None;
            neighbor.complete_witnesses.clear();
        }
    }
    if !source.valid() {
        if let Some(proof) = &mut source.proof {
            proof.bounded_block = None;
        }
        if !source.valid() {
            source.proof = None;
        }
    }
    source
}

pub fn resolve(text: &str, anchor: &Anchor) -> Resolved {
    let safe = grapheme_range(text, anchor.source_byte);
    let fallback = |reason: &str| Resolved {
        exact: false,
        source_byte: safe.start,
        grapheme_range: safe.clone(),
        tier: if text.is_empty() {
            Precision::Invalid
        } else {
            Precision::Approximate
        }
        .as_str()
        .into(),
        evidence: serde_json::json!({"method":reason,"candidate_count":0}),
    };
    if !anchor.valid() {
        return fallback("invalid bounded anchor metadata");
    }
    let unchanged = resolve_unchanged(text, anchor);
    if unchanged.exact {
        return unchanged;
    }
    let Some(p) = &anchor.proof else {
        return fallback("old record has no verified relocation proof");
    };
    if text.len() > MAX_SOURCE_BYTES || blocks(text).len() > MAX_BLOCKS {
        return fallback("relocation source index budget");
    }
    let mut source = anchor.clone();
    source.proof = None;
    let bundle = Bundle {
        source,
        window: format!("{}{}{}", anchor.before, anchor.grapheme, anchor.after),
        target: anchor.before.len()..anchor.before.len() + anchor.grapheme.len(),
        proofs: p.proofs.clone(),
        seeds: p.seeds.clone(),
        heading: p.heading.clone(),
        global_witnesses: p.global_witnesses.clone(),
        old_window_count: p.old_window_count,
        block: p.block.clone(),
        neighbors: p.neighbors.clone(),
        scopes: p.scopes.clone(),
        block_identity: p.block_identity.clone(),
        bounded_block: p.bounded_block.clone(),
    };
    let decision = resolve_bundle(text, &bundle);
    let range = grapheme_range(text, decision.source_byte);
    let exact = decision.exact
        && range.start == decision.source_byte
        && !range.is_empty()
        && text[range.clone()] == anchor.grapheme;
    let mut evidence = decision.evidence;
    if !evidence.is_object() {
        evidence = serde_json::json!({})
    }
    evidence["method"] = serde_json::json!(decision.reason);
    evidence["candidate_count"] = serde_json::json!(decision.candidates.len());
    evidence["candidate_source_bytes"] = serde_json::json!(decision.candidates);
    let mut result = Resolved {
        exact,
        source_byte: range.start,
        grapheme_range: range,
        tier: if exact {
            Precision::Text
        } else if text.is_empty() {
            Precision::Invalid
        } else {
            Precision::Approximate
        }
        .as_str()
        .into(),
        evidence,
    };
    if !result.exact {
        let (byte, tier, method) = degraded_location(text, anchor);
        result.grapheme_range = grapheme_range(text, byte);
        result.source_byte = result.grapheme_range.start;
        result.tier = tier.as_str().into();
        result.evidence["fallback_method"] = serde_json::json!(method);
    }
    result
}

/// A lower tier never claims correspondence of the deleted/ambiguous glyph.
/// Prefer a verified surviving neighboring block, then a unique heading path;
/// the final proportional fallback is explicitly approximate.
fn degraded_location(text: &str, anchor: &Anchor) -> (usize, Precision, &'static str) {
    if text.is_empty() {
        return (0, Precision::Invalid, "empty-document-top");
    }
    if let Some(proof) = &anchor.proof {
        let mut neighbors = Vec::new();
        for neighbor in &proof.neighbors {
            if work_cancelled() {
                return (0, Precision::Invalid, "cancelled");
            }
            if neighbor.count != 1 || neighbor.text.chars().count() < 12 {
                continue;
            }
            let (positions, truncated) = occurrences(text, &neighbor.text, 2);
            if !truncated && positions.len() == 1 {
                neighbors.push((
                    neighbor.source_range.start.abs_diff(anchor.source_byte),
                    positions[0],
                ));
            }
        }
        if let Some((_, byte)) = neighbors.into_iter().min_by_key(|(distance, _)| *distance)
            && let Some(block) = blocks(text).into_iter().find(|b| b.contains(&byte))
        {
            return (
                block.start,
                Precision::Block,
                "unique-surviving-neighbor-block",
            );
        }
        if !proof.heading.is_empty() {
            let matches: Vec<_> = headings(text)
                .into_iter()
                .filter(|(_, path)| *path == proof.heading)
                .collect();
            if matches.len() == 1 {
                return (
                    matches[0].0,
                    Precision::Section,
                    "unique-surviving-heading-path",
                );
            }
        }
    }
    if anchor.source_len == 0 {
        (0, Precision::Invalid, "no-valid-original-position")
    } else {
        (
            ((anchor.source_byte as u128 * text.len() as u128) / anchor.source_len as u128)
                .min(text.len() as u128) as usize,
            Precision::Approximate,
            "document-ratio-without-correspondence",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anchors_are_extended_graphemes_not_scalar_or_byte_fragments() {
        for word in ["中", "e\u{301}", "👩🏽‍💻", "🇨🇳", "☀️"] {
            let text = format!("prefix {word} suffix");
            let start = "prefix ".len();
            for offset in 0..word.len() {
                let anchor = capture(&text, start + offset);
                assert_eq!(anchor.grapheme, word);
                assert_eq!(anchor.grapheme_range, start..start + word.len());
                assert!(resolve(&text, &anchor).exact);
            }
        }
    }
    #[test]
    fn stage_a_never_calls_content_change_text_exact() {
        let anchor = capture("before original after", 9);
        assert!(!resolve("inserted before original after", &anchor).exact);
        assert_eq!(resolve("", &anchor).tier, "invalid");
    }
}

#[cfg(test)]
mod relocation_tests {
    use super::*;
    #[test]
    fn fixed_boundary_folding_preserves_every_path_candidate_and_best_runner_up_score() {
        struct Oracle<'a> {
            old: &'a [u8],
            new: &'a [u8],
            target: usize,
            mandatory: BTreeMap<usize, usize>,
        }
        impl Oracle<'_> {
            fn paths(
                &self,
                at: (usize, usize),
                state: u8,
                score: i32,
                mapping: i32,
                out: &mut Vec<(i32, i32)>,
            ) {
                let (i, j) = at;
                if i == self.old.len() && j == self.new.len() {
                    out.push((score, mapping));
                    return;
                }
                if i < self.old.len() && !self.mandatory.contains_key(&i) {
                    self.paths(
                        (i + 1, j),
                        1,
                        score - if state == 1 { 1 } else { 8 },
                        if i == self.target { -1 } else { mapping },
                        out,
                    );
                }
                if j < self.new.len() {
                    self.paths(
                        (i, j + 1),
                        2,
                        score - if state == 2 { 1 } else { 8 },
                        mapping,
                        out,
                    );
                }
                if i < self.old.len()
                    && j < self.new.len()
                    && self.mandatory.get(&i).is_none_or(|n| *n == j)
                {
                    let equal = self.old[i] == self.new[j];
                    self.paths(
                        (i + 1, j + 1),
                        0,
                        score + if equal { 4 } else { -5 },
                        if i == self.target {
                            if equal { j as i32 } else { -1 }
                        } else {
                            mapping
                        },
                        out,
                    );
                }
            }
        }
        for (old, new, target) in [
            ("abc", "abc", 1),
            ("abc", "axbc", 1),
            ("xx", "x", 0),
            ("xx", "x", 1),
            ("abc", "", 1),
            ("aab", "ab", 0),
            ("ab", "ba", 0),
        ] {
            let full_old = format!("PQ{old}RS");
            let full_new = format!("PQ{new}RS");
            let mandatory = BTreeMap::from([
                (0, 0),
                (1, 1),
                (full_old.len() - 2, full_new.len() - 2),
                (full_old.len() - 1, full_new.len() - 1),
            ]);
            let mut full = vec![];
            Oracle {
                old: full_old.as_bytes(),
                new: full_new.as_bytes(),
                target: target + 2,
                mandatory,
            }
            .paths((0, 0), 0, 0, -2, &mut full);
            let mut folded = vec![];
            Oracle {
                old: old.as_bytes(),
                new: new.as_bytes(),
                target,
                mandatory: BTreeMap::new(),
            }
            .paths((0, 0), 0, 0, -2, &mut folded);
            for (score, mapping) in &mut folded {
                *score += 16;
                if *mapping >= 0 {
                    *mapping += 2;
                }
            }
            full.sort_unstable();
            folded.sort_unstable();
            assert_eq!(
                full, folded,
                "Every valid affine path, score and candidate must correspond: {old:?}/{new:?}"
            );
            let scores = |paths: &[(i32, i32)]| {
                paths
                    .iter()
                    .map(|p| p.0)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .rev()
                    .take(2)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                scores(&full),
                scores(&folded),
                "Best and distinct runner-up scores, hence their margin, are invariant"
            );
            let candidates = |paths: &[(i32, i32)]| {
                let best = paths.last().unwrap().0;
                paths
                    .iter()
                    .filter(|p| p.0 == best)
                    .map(|p| p.1)
                    .collect::<BTreeSet<_>>()
            };
            assert_eq!(
                candidates(&full),
                candidates(&folded),
                "All-best ambiguity and deletions are invariant"
            );
            let prefix = "Immutable prefix boundary. ";
            let suffix = " Immutable suffix boundary.";
            let whole_old = format!("{prefix}{old}{suffix}");
            let whole_new = format!("{prefix}{new}{suffix}");
            let required = [
                (0..prefix.len(), 0..prefix.len()),
                (
                    whole_old.len() - suffix.len()..whole_old.len(),
                    whole_new.len() - suffix.len()..whole_new.len(),
                ),
            ];
            let project = |r: Option<DiffResult>| r.map(|r| (r.byte, r.hunk, r.score));
            assert_eq!(
                project(forced_diff_with_fold(
                    &whole_old,
                    &whole_new,
                    prefix.len() + target,
                    true,
                    &required,
                    false
                )),
                project(forced_diff_with_fold(
                    &whole_old,
                    &whole_new,
                    prefix.len() + target,
                    true,
                    &required,
                    true
                ))
            );
        }
    }
    #[test]
    fn repeated_identifiers_inside_logical_nodes_keep_trimmed_complete_witnesses() {
        for separator in ["\n", "\n\n"] {
            for cluster in ["中", "e\u{301}", "👩🏽‍💻", "🇨🇳", "👍🏿", "🙂"] {
                let old = (0..18).map(|i| {
                    let unit=format!("Original record-{i:04}: {cluster} has immutable content and a repeated identifier. One visible sentence preserves each original position while the surrounding paragraph may split and join its preceding content. ");
                    assert!(unit.graphemes(true).count()*9>1536);
                    format!("{}{separator}",unit.repeat(3))
                }).collect::<String>();
                let line = old.find("Original record-0009:").unwrap();
                let first = old[line..].find(cluster).unwrap() + line;
                let target =
                    old[first + cluster.len()..].find(cluster).unwrap() + first + cluster.len();
                let anchor = capture(&old, target);
                let scalar = old[..target].chars().count();
                let at = |n| old.char_indices().nth(n).map_or(old.len(), |(b, _)| b);
                let mut changed = old.clone();
                changed.insert_str(at(scalar + cluster.chars().count() + 24), "\n\n");
                changed.insert_str(at(scalar - 24), "\n\n");
                let previous = changed[..at(scalar - 50)].rfind(separator).unwrap();
                changed.replace_range(previous..previous + separator.len(), " ");
                let found = resolve(&changed, &anchor);
                assert!(found.exact, "{separator:?} {cluster} {found:?}");
                assert_eq!(found.source_byte, target + 2 + 1 - separator.len());
                let end = old[line..].find(separator).unwrap() + line + separator.len();
                let mut deleted = old.clone();
                deleted.replace_range(line..end, "");
                assert!(
                    !resolve(&deleted, &anchor).exact,
                    "Removed original repeated node must not map to an identical neighboring body"
                );
            }
        }
    }
    #[test]
    fn split_both_sides_of_a_repeated_middle_with_merged_predecessor_uses_complete_scope() {
        for separator in ["\n", "\n\n"] {
            for cluster in ["中", "e\u{301}", "👩🏽‍💻", "🇨🇳", "👍🏿", "🙂"] {
                let sentence =
                    format!("Repeated body {cluster} still carries the original text identity. ");
                let old = (0..18)
                    .map(|i| {
                        format!(
                            "Record {i:04} immutable-{i:04}: {}{separator}",
                            sentence.repeat(9)
                        )
                    })
                    .collect::<String>();
                let line = old.find("Record 0009 ").unwrap();
                let first = old[line..].find(cluster).unwrap() + line;
                let target =
                    old[first + cluster.len()..].find(cluster).unwrap() + first + cluster.len();
                let anchor = capture(&old, target);
                assert!(anchor.valid());
                let scalar = old[..target].chars().count();
                let byte_at = |n| old.char_indices().nth(n).map_or(old.len(), |(b, _)| b);
                let mut changed = old.clone();
                changed.insert_str(byte_at(scalar + cluster.chars().count() + 24), "\n\n");
                changed.insert_str(byte_at(scalar - 24), "\n\n");
                let previous = changed[..byte_at(scalar - 50)].rfind(separator).unwrap();
                changed.replace_range(previous..previous + separator.len(), " ");
                let expected = target + 2 + 1 - separator.len();
                let found = resolve(&changed, &anchor);
                assert!(found.exact, "{separator:?} {cluster}: {found:?}");
                assert_eq!(found.source_byte, expected, "{separator:?} {cluster}");
                let end = old[line..].find(separator).unwrap() + line + separator.len();
                let mut deleted = old.clone();
                deleted.replace_range(line..end, "");
                assert!(
                    !resolve(&deleted, &anchor).exact,
                    "Deleted repeated node: {separator:?} {cluster}"
                );
            }
        }
    }
    #[test]
    fn original_three_node_scope_survives_repeated_internal_ids_and_two_boundary_edits() {
        for cluster in ["中", "🙂", "e\u{301}", "👩🏽‍💻", "🇨🇳", "👍🏿"] {
            let mut old = String::new();
            for i in 0..18 {
                let unit = format!(
                    "Original paragraph identity {i:04}: ordinary information {cluster} and recurring tail content. "
                );
                old.push_str(&format!(
                    "## Original section {i:04}\n\n{}\n\n",
                    unit.repeat(3)
                ));
            }
            let start = old.find("Original paragraph identity 0009").unwrap();
            let end = start + old[start..].find("\n\n").unwrap();
            let byte = start + old[start..end].rfind(cluster).unwrap();
            let anchor = capture(&old, byte);
            let merge = old[..start].rfind("\n\n").unwrap();
            let split = end + 2 + old[end + 2..].char_indices().nth(4).unwrap().0;
            let mut new = old.clone();
            new.insert_str(split, "\n\n");
            new.replace_range(merge..merge + 2, " ");
            let result = resolve(&new, &anchor);
            assert!(result.exact, "{cluster}: {result:?}");
            assert_eq!(result.source_byte, byte - 1, "{cluster}: {result:?}");
            let mut deleted = old.clone();
            deleted.replace_range(start..end + 2, "");
            assert!(
                !resolve(&deleted, &anchor).exact,
                "{cluster}: deleted body matched neighbor"
            );
        }
    }
    #[test]
    fn locally_unique_structure_can_identify_a_changed_body_shared_by_many_sections() {
        let common = format!(
            "{}核心目标🙂仍在原段并保持真实对应。{}",
            "共有前文描述用于保持阅读内容稳定。".repeat(18),
            "相同后文在多个章节独立存在。".repeat(18)
        );
        let sections=(0..7).map(|i|format!("## Verifiable chapter {i}\n\n前置唯一凭据代码={i:04}。\n\n{common}\n\n后置唯一凭据代码={i:04}。\n\n")).collect::<Vec<_>>();
        let old = sections.concat();
        let start = sections[..3].iter().map(String::len).sum::<usize>();
        let byte = start + sections[3].find("🙂仍在原段").unwrap();
        let anchor = capture(&old, byte);
        for left in [true, false] {
            let (a, b) = if left {
                let before = &old[..byte];
                (
                    before.char_indices().rev().nth(29).unwrap().0,
                    before.char_indices().rev().nth(7).unwrap().0,
                )
            } else {
                (
                    byte + 4 + old[byte + 4..].char_indices().nth(8).unwrap().0,
                    byte + 4 + old[byte + 4..].char_indices().nth(30).unwrap().0,
                )
            };
            let replacement = "改写附近说明而保留真实目标。";
            let mut new = old.clone();
            new.replace_range(a..b, replacement);
            let expected = if left {
                byte + replacement.len() - (b - a)
            } else {
                byte
            };
            let decision = resolve(&new, &anchor);
            assert!(decision.exact, "{decision:?}");
            assert_eq!(decision.source_byte, expected);
        }
        let mut deleted = old.clone();
        deleted.replace_range(byte..byte + 4, "");
        assert!(!resolve(&deleted, &anchor).exact);
        let without = sections
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 3)
            .map(|(_, s)| s.as_str())
            .collect::<String>();
        assert!(!resolve(&without, &anchor).exact);
    }
    #[test]
    fn short_heading_uses_complete_node_and_surviving_neighbor_not_common_title_alone() {
        let old=(0..80).map(|i|format!("## 操作 {i:03}\n\nUnique original paragraph identity-{i:03} has independent stable information and a long enough source neighbor.\n\n")).collect::<String>();
        let byte = old.find("操作 043").unwrap() + 3;
        let anchor = capture(&old, byte);
        assert!(
            anchor
                .proof
                .as_ref()
                .unwrap()
                .block_identity
                .as_ref()
                .unwrap()
                .length
                < 64
        );
        let prefix = "New introductory material. ".repeat(600);
        let new = format!("{prefix}\n\n{old}");
        let decision = resolve(&new, &anchor);
        assert!(decision.exact, "{decision:?}");
        assert_eq!(decision.source_byte, byte + prefix.len() + 2);
        assert!(
            !resolve(
                "# Unrelated manuscript\n\n操作 043\n\nEntirely different information.",
                &anchor
            )
            .exact
        );
        let mut deleted = old.clone();
        let start = old[..byte].rfind("##").unwrap();
        let end = byte + old[byte..].find("\n\n").unwrap();
        deleted.replace_range(start..end, "");
        assert!(!resolve(&deleted, &anchor).exact);
    }
    #[test]
    fn unique_complete_block_witness_recovers_local_rewrite_with_repeated_right_context() {
        let pattern = "Shared **visible wording** and repeated explanatory source with 中文 é 👩🏽‍💻 within the same paragraph. ";
        let old = (0..70)
            .map(|i| format!("Original record identity-{i:03}: {}\n\n", pattern.repeat(4)))
            .collect::<String>();
        let block = old.find("identity-037").unwrap();
        let first = block + old[block..].find(pattern).unwrap();
        let byte = first + pattern.len() + pattern.find("visible").unwrap();
        let anchor = capture(&old, byte);
        assert!(anchor.proof.as_ref().unwrap().bounded_block.is_some());
        for (left, right) in [(byte - 70, byte - 8), (byte + 18, byte + 80)] {
            let mut start = left;
            while !old.is_char_boundary(start) {
                start -= 1;
            }
            let mut end = right;
            while !old.is_char_boundary(end) {
                end -= 1;
            }
            let replacement = "A rewritten local span with substantial new wording ";
            let mut new = old.clone();
            new.replace_range(start..end, replacement);
            let expected = if end <= byte {
                byte + replacement.len() - (end - start)
            } else {
                byte
            };
            let decision = resolve(&new, &anchor);
            assert!(decision.exact, "{decision:?}");
            assert_eq!(decision.source_byte, expected);
        }
        let start = old[..byte].rfind("\n\n").unwrap() + 2;
        let end = byte + old[byte..].find("\n\n").unwrap();
        let mut deleted = old.clone();
        deleted.replace_range(start..end, "");
        assert!(!resolve(&deleted, &anchor).exact);
        let mut repeated = old.clone();
        repeated.replace_range(first..first + pattern.len(), "");
        assert!(
            !resolve(&repeated, &anchor).exact,
            "Deleting one indistinguishable repeat cannot preserve the original selected repeat with certainty"
        );
    }
    #[test]
    fn complete_node_scope_can_outvote_a_truncated_repeating_context_window() {
        let common =
            "An explanation shared across records: 中文 é 👩🏽‍💻 recurring words and source content. "
                .repeat(7);
        let old = (0..96)
            .map(|i| format!("Record-{i:04} immutable identity. {common} ending!\n\n"))
            .collect::<String>();
        let node = old.find("Record-0047").unwrap();
        let byte = node + old[node..].find("中文").unwrap() + common.len() / 2;
        let byte = grapheme_range(&old, byte).start;
        let anchor = capture(&old, byte);
        let proof = anchor.proof.as_ref().unwrap();
        assert!(proof.scopes.iter().any(|scope| scope.old_count > 1));
        assert!(
            proof
                .block_identity
                .as_ref()
                .unwrap()
                .scope_counts
                .contains(&1)
        );
        let mut new = old.clone();
        let cut = old[..node].rfind("\n\n").unwrap();
        new.replace_range(cut..cut + 2, "");
        let expected = byte - 2;
        let range = grapheme_range(&new, expected);
        new.insert_str(range.end, "\n\n");
        let result = resolve(&new, &anchor);
        assert!(result.exact, "{result:?}");
        assert_eq!(result.source_byte, expected);
    }

    #[test]
    fn source_corridor_requires_every_saved_literal_bridge_and_consistent_overlap() {
        let common =
            "A common paragraph containing repeated source words and Unicode 中文 é. ".repeat(5);
        let old = (0..12)
            .map(|i| format!("Node-{i:04} original identity. {common} ending!\n\n"))
            .collect::<String>();
        let node = old.find("Node-0006").unwrap();
        let byte = node + old[node..].find("ending!").unwrap();
        let mut bundle = capture_bundle(&old, byte, fingerprint(&old)).unwrap();
        let block = bundle.bounded_block.as_ref().unwrap().source_range.clone();
        let left = bundle
            .neighbors
            .iter()
            .filter(|n| n.block_range.end <= block.start)
            .max_by_key(|n| n.block_range.end)
            .unwrap();
        let right_index = bundle
            .neighbors
            .iter()
            .enumerate()
            .filter(|(_, n)| n.block_range.start >= block.end)
            .max_by_key(|(_, n)| n.block_range.start)
            .unwrap()
            .0;
        let range = left.source_range.start..bundle.neighbors[right_index].source_range.end;
        assert_eq!(
            saved_source_corridor(&bundle, range.clone()).as_deref(),
            Some(&old[range.clone()])
        );
        let saved_gap = bundle.neighbors[right_index].adjacent_gap.take();
        assert!(saved_gap.is_some());
        assert!(
            saved_source_corridor(&bundle, range.clone()).is_none(),
            "Legacy proofs cannot invent an omitted whitespace bridge"
        );
        bundle.neighbors[right_index].adjacent_gap = saved_gap;
        bundle
            .window
            .replace_range(bundle.target.start..bundle.target.start + 1, "X");
        assert!(
            saved_source_corridor(&bundle, range).is_none(),
            "Conflicting saved fragments cannot produce exact evidence"
        );
    }

    #[test]
    fn bounded_metadata_rejects_impossible_original_size_and_context_extent() {
        let text = doc();
        let mut anchor = capture(&text, text.find("paragraph-030").unwrap());
        assert!(anchor.valid());
        anchor.source_len = MAX_SOURCE_BYTES + 1;
        assert!(!anchor.valid());
        let mut anchor = capture(&text, text.find("paragraph-030").unwrap());
        anchor.source_len = anchor.grapheme_range.end;
        assert!(!anchor.valid());
        let mut serialized =
            serde_json::to_value(capture(&text, text.find("paragraph-030").unwrap())).unwrap();
        serialized["proof"]
            .as_object_mut()
            .unwrap()
            .remove("bounded_block");
        let old: Anchor = serde_json::from_value(serialized).unwrap();
        assert!(
            old.valid(),
            "Earlier schema-3 proofs safely retain their smaller evidence set"
        );
        let mut anchor = capture(&text, text.find("paragraph-030").unwrap());
        let proof = anchor.proof.as_mut().unwrap();
        let neighbor = proof
            .neighbors
            .iter_mut()
            .find(|n| n.complete.is_some())
            .unwrap();
        neighbor.complete.as_mut().unwrap().push('X');
        assert!(
            !anchor.valid(),
            "Complete-node evidence must match its saved range and fragment"
        );
        let mut anchor = capture(&text, text.find("paragraph-030").unwrap());
        let proof = anchor.proof.as_mut().unwrap();
        let neighbor = proof
            .neighbors
            .iter_mut()
            .find(|n| !n.complete_witnesses.is_empty())
            .unwrap();
        neighbor.complete_witnesses[0].range.end = usize::MAX;
        assert!(
            !anchor.valid(),
            "Complete-node witness coordinates must be bounded before slicing"
        );
    }
    fn doc() -> String {
        format!("# Alpha\n\n{}\n\n## Target\n\n{}\n\n# Omega\n\n{}",(0..80).map(|i|format!("Original introduction {i} different context. ")).collect::<String>(),(0..60).map(|i|format!("Unique paragraph-{i:03} 中文 é 👩🏽‍💻 contains original distinct information and a meaningful sentence.\n\n")).collect::<String>(),(0..80).map(|i|format!("Original conclusion {i} unique ending. ")).collect::<String>())
    }
    #[test]
    fn transformations() {
        let old = doc();
        let byte = old.find("paragraph-030").unwrap() + 3;
        let a = capture(&old, byte);
        for new in [
            format!(
                "{}{}",
                "Substantial new introductory material. ".repeat(500),
                old
            ),
            old[old.find("## Target").unwrap()..].to_string(),
            {
                let middle = old.find("## Target").unwrap();
                format!("{}{}", &old[middle..], &old[..middle])
            },
            old.replace("## Target", "# New renamed section"),
        ] {
            let r = resolve(&new, &a);
            assert!(r.exact, "{r:?}");
            assert_eq!(r.source_byte, new.find("paragraph-030").unwrap() + 3);
        }
    }

    #[test]
    fn moving_and_splitting_one_paragraph() {
        let old = doc();
        let byte = old.find("paragraph-030").unwrap() + 3;
        let a = capture(&old, byte);
        let start = old[..byte].rfind("\n\n").unwrap() + 2;
        let end = byte + old[byte..].find("\n\n").unwrap();
        let paragraph = &old[start..end];
        let mut moved = old.clone();
        moved.replace_range(start..end, "");
        moved.push_str("\n\n");
        moved.push_str(paragraph);
        let r = resolve(&moved, &a);
        assert!(r.exact, "{r:?}");
        assert_eq!(r.source_byte, moved.rfind("paragraph-030").unwrap() + 3);
        let mut split = old.clone();
        split.insert_str(byte + 18, "\n\n");
        let r = resolve(&split, &a);
        assert!(r.exact, "{r:?}");
        assert_eq!(r.source_byte, byte);
    }
    #[test]
    fn local_rewrite_uses_forced_diff() {
        let old = doc();
        let byte = old.find("paragraph-030").unwrap() + 3;
        let a = capture(&old, byte);
        let start = byte - 45;
        let end = byte - 6;
        let mut new = old.clone();
        new.replace_range(
            start..end,
            "A substantially rewritten nearby phrase, retaining the selected original character: ",
        );
        let expected = byte
            + "A substantially rewritten nearby phrase, retaining the selected original character: "
                .len() - (end - start);
        let r = resolve(&new, &a);
        assert!(r.exact, "{r:?}");
        assert_eq!(r.source_byte, expected);
    }
    #[test]
    fn symmetric_and_deleted_never_exact() {
        let old = doc();
        let byte = old.find("paragraph-030").unwrap() + 3;
        let a = capture(&old, byte);
        let duplicated = format!("{old}\n{old}");
        assert!(!resolve(&duplicated, &a).exact);
        let mut deleted = old.clone();
        let start = old[..byte].rfind("\n\n").unwrap();
        let end = byte + old[byte..].find("\n\n").unwrap();
        deleted.replace_range(start..end, "");
        assert!(
            !resolve(&deleted, &a).exact,
            "deleted returned {:?}",
            resolve(&deleted, &a)
        );
        let mut unrelated = String::from("# Entirely new unrelated manuscript\n\n");
        unrelated.push_str("contains original distinct information and a meaningful sentence.");
        assert!(!resolve(&unrelated, &a).exact);
    }
    #[test]
    fn adjacent_same_initial_deletion_not_false_exact() {
        let old = doc().replace("Unique paragraph-030", "Unique target text paragraph-030");
        let byte = old.find("target text").unwrap();
        let a = capture(&old, byte);
        let new = old.replacen("target text", "text", 1);
        assert!(!resolve(&new, &a).exact);
    }
    #[test]
    fn renamed_sections_with_repeated_body_need_unique_neighbor_scope() {
        let common=(0..25).map(|i|format!("Shared paragraph sentence-{i:03} has stable content duplicated across all four sections. ")).collect::<String>();
        let mut old = String::new();
        for i in 0..4 {
            old.push_str(&format!("# Original heading {i}\n\nUnique stable preceding block identity {i} has enough original information.\n\n{common}\n\nUnique stable following block identity {i} provides independent context.\n\n"));
        }
        let start = old.find("# Original heading 2").unwrap();
        let byte = start + old[start..].find(&common).unwrap() + 430;
        let a = capture(&old, byte);
        assert!(a.proof.as_ref().unwrap().old_window_count > 1);
        let new = old.replace("Original heading", "Entirely renamed section");
        let expected = byte + 3 * ("Entirely renamed section".len() - "Original heading".len());
        let r = resolve(&new, &a);
        assert!(r.exact, "{r:?}");
        assert_eq!(r.source_byte, expected);
    }
    #[test]
    fn deleted_block_with_long_common_opening_must_not_map_to_successor() {
        let mut old = String::from("# Document\n\n");
        for i in 0..60 {
            old.push_str(&format!(
                "{} distinct original record identity-{i:03} finishes this paragraph.\n\n",
                "The same long boilerplate opening and shared sentences. ".repeat(15)
            ));
        }
        let phrase = "distinct original record identity-030";
        let end_id = old.find(phrase).unwrap();
        let start = old[..end_id].rfind("\n\n").unwrap() + 2;
        let end = end_id + old[end_id..].find("\n\n").unwrap();
        let a = capture(&old, start + 3);
        let mut new = old.clone();
        new.replace_range(start..end + 2, "");
        let r = resolve(&new, &a);
        assert!(
            !r.exact,
            "deleted original block mapped to surviving common prefix: {r:?}"
        );
    }
    #[test]
    fn whole_identical_long_block_proves_position_in_repeating_text() {
        let old = format!(
            "# Heading\n\nUnique preceding paragraph marks the original complete body.\n\n{}\n\nUnique following paragraph keeps separate context.",
            "Repeated wording inside the same source paragraph with no unique short local window. "
                .repeat(80)
        );
        let byte = old.find("Repeated wording").unwrap() + 2000;
        let a = capture(&old, byte);
        let new = format!(
            "{}{}",
            "Substantial inserted front material. ".repeat(400),
            old
        );
        let r = resolve(&new, &a);
        assert!(r.exact, "{r:?}");
        assert_eq!(
            r.source_byte,
            byte + "Substantial inserted front material. ".len() * 400
        );
        let mut deleted = old.clone();
        let block_start = old.find("Repeated wording").unwrap();
        let block_end = old.rfind("\n\nUnique following").unwrap();
        deleted.replace_range(block_start..block_end, "");
        assert!(!resolve(&deleted, &a).exact);
    }
    #[test]
    fn split_tail_and_merged_predecessor_force_the_internal_unique_witness() {
        for cluster in ["中", "🙂", "e\u{301}", "👩🏽‍💻", "🇨🇳", "👍🏿"] {
            let recurring = format!(
                "Common explanatory material preserves recurring content across paragraphs {cluster} tail. "
            );
            let mut old = String::new();
            for i in 0..18 {
                old.push_str(&format!(
                    "段落 {i:05} identity-{i:04}: {}\n\n",
                    recurring.repeat(5)
                ));
            }
            let start = old.find("段落 00009 identity-0009").unwrap();
            let end = start + old[start..].find("\n\n").unwrap();
            let byte = start + old[start..end].rfind(cluster).unwrap();
            let anchor = capture(&old, byte);
            let split_before = old[..byte].char_indices().rev().nth(23).unwrap().0;
            // Break even the neighbor's first eight graphemes; an internal
            // old-and-new-unique fragment must constrain the extended diff.
            let split_after = end + 2 + old[end + 2..].char_indices().nth(4).unwrap().0;
            let merge = old[..split_before].rfind("\n\n").unwrap();
            let mut new = old.clone();
            new.insert_str(split_after, "\n\n");
            new.insert_str(split_before, "\n\n");
            new.replace_range(merge..merge + 2, " ");
            let result = resolve(&new, &anchor);
            assert!(result.exact, "{cluster}: {result:?}");
            assert_eq!(result.source_byte, byte + 1, "{cluster}: {result:?}");
            let mut deleted = old.clone();
            deleted.replace_range(start..end + 2, "");
            assert!(
                !resolve(&deleted, &anchor).exact,
                "{cluster}: deleted node matched a copy"
            );
        }
    }
    #[test]
    fn short_heading_closes_complete_scope_for_split_node_edges() {
        for cluster in ["中", "🙂", "e\u{301}", "👩🏽‍💻", "🇯🇵", "👍🏿"] {
            let rows: Vec<_> = (0..18).map(|i| format!(
                "{cluster} item {i:04} immutable id-{i:04} {} ending {cluster}\n\n## H{i:04}\n\n",
                "Shared explanatory body contains repeated common context and harmless prose. ".repeat(7)
            )).collect();
            let old = rows.concat();
            let start = rows[..9].iter().map(String::len).sum::<usize>();
            let end = start + rows[9].find("\n\n").unwrap();
            for target in [start, end - cluster.len()] {
                let anchor = capture(&old, target);
                assert!(
                    anchor
                        .proof
                        .as_ref()
                        .unwrap()
                        .neighbors
                        .iter()
                        .any(|n| n.text.len() < 24)
                );
                let left = old[..target].char_indices().rev().nth(23).unwrap().0;
                let after = target + cluster.len();
                let right = after + old[after..].char_indices().nth(24).unwrap().0;
                let merge = old[..old[..target].char_indices().rev().nth(49).unwrap().0]
                    .rfind("\n\n")
                    .unwrap();
                let mut new = old.clone();
                new.insert_str(right, "\n\n");
                new.insert_str(left, "\n\n");
                new.replace_range(merge..merge + 2, " ");
                let result = resolve(&new, &anchor);
                assert!(result.exact, "{cluster} {target}: {result:?}");
                assert_eq!(
                    result.grapheme_range,
                    target + 1..target + 1 + cluster.len()
                );
                let removed = old[..start].to_owned() + &old[end..];
                assert!(!resolve(&removed, &anchor).exact, "Deleted node {cluster}");
                assert!(
                    !resolve(&new.repeat(2), &anchor).exact,
                    "Symmetric node {cluster}"
                );

                // Legacy records cannot be assigned missing short-node proof.
                // Drop that metadata without inventing smaller old seed counts.
                let mut legacy = anchor.clone();
                let proof = legacy.proof.as_mut().unwrap();
                proof.neighbors.retain(|n| n.text.len() >= 24);
                proof.scopes.clear();
                proof.block_identity.as_mut().unwrap().scope_counts.clear();
                assert!(legacy.valid());
                assert!(
                    !resolve(&new, &legacy).exact,
                    "Unproven legacy scope {cluster}"
                );
            }
        }
    }

    #[test]
    fn short_heading_witness_requires_unique_actual_new_parser_node() {
        let text = "# First heading\n\nBefore original material repeated for reliable nonlocal context.\n\nTarget paragraph with original important text to capture and preserve.\n\n## H900\n\nAfter original material repeated for reliable nonlocal context.\n";
        let target = text.find("Target").unwrap();
        let anchor = capture(text, target);
        let short = anchor
            .proof
            .as_ref()
            .unwrap()
            .neighbors
            .iter()
            .find(|n| n.text == "## H900\n")
            .unwrap();
        let good = "\n\n## H900\n\n";
        assert_eq!(
            neighbor_occurrences(good, short, &blocks(good), &headings(good))
                .0
                .len(),
            1
        );
        for invalid in [
            "```text\n## H900\n```\n",
            "\\## H900\n",
            "## H900\n\n## H900\n",
        ] {
            assert!(
                neighbor_occurrences(invalid, short, &blocks(invalid), &headings(invalid))
                    .0
                    .is_empty()
            );
        }
        let mut invalid = anchor.clone();
        let node = invalid
            .proof
            .as_mut()
            .unwrap()
            .neighbors
            .iter_mut()
            .find(|n| n.text == "## H900\n")
            .unwrap();
        node.text = "ordinary".into();
        assert!(!invalid.valid());
    }
}
