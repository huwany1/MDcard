// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Independent structural adversaries, separate from the frozen 1400 gate.
//! Expected targets come from construction/permutation, never the resolver.
#![allow(dead_code)]
#[path = "../../../src/reading_precision.rs"]
mod reading_precision;

const CLUSTERS: [&str; 6] = ["中", "🙂", "e\u{301}", "👩🏽‍💻", "👍🏽", "🇨🇳"];

#[test]
fn independent_right_rewrite_removes_separator_and_next_identity_without_removing_target() {
    let mut failures = Vec::new();
    let mut samples = 0;
    for cluster in CLUSTERS {
        for separator in ["\n", "\n\n"] {
            let phrase = format!(
                "Repeated explanatory words describe reading and source boundaries. \
                 The same source passage appears three times inside every logical node. \
                 Additional immutable words retain their original order. Target {cluster}ending!"
            );
            let rows = (0..96)
                .map(|n| format!("unique-node-{n:05}: {}{separator}", phrase.repeat(3)))
                .collect::<Vec<_>>();
            let old = rows.concat();
            let start = rows[..48].iter().map(String::len).sum::<usize>();
            let end = start + rows[48].len();
            let byte = start + rows[48].rfind(cluster).unwrap();
            let anchor = reading_precision::capture(&old, byte);
            let after = byte + cluster.len();
            let edit_start = after + old[after..].char_indices().nth(7).unwrap().0;
            let edit_end = after + old[after..].char_indices().nth(82).unwrap().0;
            assert_eq!(edit_start, end - separator.len());
            assert!(edit_end > end + "unique-node-00049: ".len());
            let mut changed = old.clone();
            changed.replace_range(
                edit_start..edit_end,
                "Revised adjacent wording retains the captured original text.",
            );
            assert_eq!(&changed[byte..byte + cluster.len()], cluster);
            let result = reading_precision::resolve(&changed, &anchor);
            samples += 1;
            if !result.exact || result.grapheme_range != (byte..byte + cluster.len()) {
                failures.push(format!("survivor {cluster} separator={separator:?}: expected={byte}, actual={} exact={} {:?}", result.source_byte, result.exact, result.evidence));
            }
            for (kind, negative) in [
                ("deleted", format!("{}{}", &old[..start], &old[end..])),
                ("symmetric", changed.repeat(2)),
            ] {
                let result = reading_precision::resolve(&negative, &anchor);
                samples += 1;
                if result.exact {
                    failures.push(format!(
                        "false exact {kind} {cluster} separator={separator:?}: {result:?}"
                    ));
                }
            }
        }
    }
    eprintln!(
        "Independent lost-next-identity supplement samples={samples}; failures={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn independent_common_interior_and_lost_next_identity_use_complete_original_evidence() {
    let mut failures = Vec::new();
    let mut truncated_windows = 0;
    let mut samples = 0;
    for cluster in CLUSTERS {
        for separator in ["\n", "\n\n"] {
            let phrase = format!(
                "Repeated explanatory words describe reading and source boundaries. \
                 The same source passage appears three times inside every logical node. \
                 Additional immutable words retain their original order. Target {cluster}end"
            );
            let rows = (0..96)
                .map(|n| format!("unique-node-{n:05}: {}{separator}", phrase.repeat(3)))
                .collect::<Vec<_>>();
            let old = rows.concat();
            let row_start = rows[..48].iter().map(String::len).sum::<usize>();
            let row_end = row_start + rows[48].len();
            for copy in 0..3 {
                let byte = row_start + rows[48].match_indices(cluster).nth(copy).unwrap().0;
                let anchor = reading_precision::capture(&old, byte);
                let window = format!("{}{}{}", anchor.before, cluster, anchor.after);
                if old.match_indices(&window).count() > 65 {
                    truncated_windows += 1;
                }
                for split in [false, true] {
                    let after = byte + cluster.len();
                    let mut changed = old.clone();
                    let expected = if split {
                        let left = old[..byte].char_indices().rev().nth(23).unwrap().0;
                        let right = after + old[after..].char_indices().nth(24).unwrap().0;
                        changed.insert_str(right, "\n\n");
                        changed.insert_str(left, "\n\n");
                        changed.replace_range(row_start - separator.len()..row_start, " ");
                        byte + 3 - separator.len()
                    } else {
                        let start = after + old[after..].char_indices().nth(7).unwrap().0;
                        let end = after + old[after..].char_indices().nth(82).unwrap().0;
                        changed.replace_range(
                            start..end,
                            "Revised adjacent wording retains the captured original text.",
                        );
                        byte
                    };
                    assert_eq!(&changed[expected..expected + cluster.len()], cluster);
                    let result = reading_precision::resolve(&changed, &anchor);
                    samples += 1;
                    if !result.exact
                        || result.grapheme_range != (expected..expected + cluster.len())
                    {
                        failures.push(format!("survivor {cluster} separator={separator:?} copy={copy} split={split}: expected={expected}, actual={} exact={} {:?}", result.source_byte, result.exact, result.evidence));
                    }
                    let removed = format!("{}{}", &old[..row_start], &old[row_end..]);
                    for (kind, negative) in [("deleted", removed), ("symmetric", changed.repeat(2))]
                    {
                        let result = reading_precision::resolve(&negative, &anchor);
                        samples += 1;
                        if result.exact {
                            failures.push(format!("false exact {kind} {cluster} separator={separator:?} copy={copy} split={split}: {result:?}"));
                        }
                    }
                }
            }
        }
    }
    assert!(
        truncated_windows > 0,
        "The public supplement must exercise globally repeated context windows"
    );
    eprintln!(
        "Independent common-interior supplement samples={samples}; truncated_windows={truncated_windows}; failures={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn independent_block_edges_survive_adjacent_split_merge_without_inventing_deleted_identity() {
    let mut failures = Vec::new();
    let mut samples = 0;
    for cluster in CLUSTERS {
        for separator in ["\n\n", "\r\n\r\n"] {
            for count in [24, 1800] {
                for sectioned in [false, true] {
                    let body = "Common prose **emphasis** [visible](hidden-destination) repeated inside this block. ".repeat(3);
                    let rows = (0..count)
                    .map(|n| {
                        let heading = if sectioned && n % 4 == 0 { format!("## H{}{separator}", n / 4) } else { String::new() };
                        format!("{heading}{cluster} 独立单元 {n:05} unique-unit-{n:05}: {body}block ending {cluster}{separator}")
                    })
                    .collect::<Vec<_>>();
                    let source = rows.concat();
                    // The selected row ends a section when headings are present;
                    // its next parser node is a short heading, not another body.
                    let selected = count / 2 - 1;
                    let start = rows[..selected].iter().map(String::len).sum::<usize>();
                    let end = start + rows[selected].len();
                    for at_start in [true, false] {
                        let byte = if at_start {
                            start
                        } else {
                            end - separator.len() - cluster.len()
                        };
                        let anchor = reading_precision::capture(&source, byte);
                        let left = source[..byte].char_indices().rev().nth(23).unwrap().0;
                        let after = byte + cluster.len();
                        let right = after + source[after..].char_indices().nth(24).unwrap().0;
                        let mut edits = vec![
                            (left, left, "\n\n"),
                            (right, right, "\n\n"),
                            (start - separator.len(), start, " "),
                        ];
                        let expected = edits
                            .iter()
                            .filter(|(_, b, _)| *b <= byte)
                            .fold(byte as isize, |n, (a, b, value)| {
                                n + value.len() as isize - (b - a) as isize
                            }) as usize;
                        edits.sort_by_key(|(a, _, _)| std::cmp::Reverse(*a));
                        let mut changed = source.clone();
                        for (a, b, value) in edits {
                            changed.replace_range(a..b, value);
                        }
                        assert_eq!(&changed[expected..expected + cluster.len()], cluster);
                        let answer = reading_precision::resolve(&changed, &anchor);
                        samples += 1;
                        if !answer.exact
                            || answer.tier != "text"
                            || answer.grapheme_range != (expected..expected + cluster.len())
                        {
                            failures.push(format!("survivor count={count} sectioned={sectioned} edge={at_start} sep={separator:?} {cluster}: expected={expected}, actual={} exact={} {:?}", answer.source_byte, answer.exact, answer.evidence));
                        }
                        // The same visible edge occurs in all rows. Losing the
                        // selected complete unit cannot identify another copy.
                        let removed = format!("{}{}", &source[..start], &source[end..]);
                        let missing = reading_precision::resolve(&removed, &anchor);
                        samples += 1;
                        if missing.exact {
                            failures.push(format!("deleted edge false exact count={count} sectioned={sectioned} {at_start} {separator:?} {cluster}: {:?}", missing.evidence));
                        }
                        // Duplicating the complete changed manuscript removes
                        // observable identity, including both neighboring units.
                        let ambiguous = reading_precision::resolve(&changed.repeat(2), &anchor);
                        samples += 1;
                        if ambiguous.exact {
                            failures.push(format!("symmetric edge false exact count={count} sectioned={sectioned} {at_start} {separator:?} {cluster}: {:?}", ambiguous.evidence));
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "Independent block-edge supplementary samples={samples}; failures={}",
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn chapter(index: usize, cluster: &str) -> String {
    let body = format!(
        "{}共同的阅读目标{cluster}，正文和前后局部文字在各章完全相同。{}",
        "重复的操作说明用于检验结构证据。".repeat(20),
        "相同的后文不能代替章节身份。".repeat(20)
    );
    format!(
        "## 独立章节 {index}\n\n本章前置凭据代码={index:04}。\n\n{body}\n\n本章结束凭据代码={index:04}。\n\n"
    )
}

#[test]
fn independent_repeated_paragraph_add_delete_reorder_keeps_distinguishable_section() {
    for cluster in CLUSTERS {
        let chapters = (0..9).map(|i| chapter(i, cluster)).collect::<Vec<_>>();
        let source = chapters.concat();
        let target_in_chapter = chapters[4].find(&format!("{cluster}，正文")).unwrap();
        let old_byte = chapters[..4].iter().map(String::len).sum::<usize>() + target_in_chapter;
        let anchor = reading_precision::capture(&source, old_byte);
        // Independent permutations also remove/add copies in other chapters.
        // Target chapter 4 remains; its body is never made globally unique.
        for order in [
            vec![8, 7, 4, 6, 5, 3, 2, 1, 0],
            vec![0, 4, 7],
            vec![8, 8, 4, 0, 1, 2, 8],
        ] {
            let target_index = order.iter().position(|&i| i == 4).unwrap();
            let expected = order[..target_index]
                .iter()
                .map(|&i| chapters[i].len())
                .sum::<usize>()
                + target_in_chapter;
            let new = order
                .iter()
                .map(|&i| chapters[i].as_str())
                .collect::<String>();
            let result = reading_precision::resolve(&new, &anchor);
            assert!(result.exact, "{cluster} {order:?}: {:?}", result.evidence);
            assert_eq!(result.tier, "text");
            assert_eq!(result.source_byte, expected);
            assert_eq!(result.grapheme_range, expected..expected + cluster.len());
        }
    }
}

#[test]
fn independent_symmetric_duplicate_removal_and_addition_have_no_observable_identity() {
    for cluster in CLUSTERS {
        let part = format!(
            "## 相同标题\n\n{}原阅读文字{cluster}仍有多份相同正文{}\n\n",
            "完全相同的前文。".repeat(35),
            "完全相同的后文。".repeat(35)
        );
        let old = part.repeat(8);
        let byte = part.len() * 4 + part.find(&format!("{cluster}仍有")).unwrap();
        let anchor = reading_precision::capture(&old, byte);
        for copies in [3, 5, 10, 13] {
            let result = reading_precision::resolve(&part.repeat(copies), &anchor);
            assert!(
                !result.exact,
                "Symmetric {copies}-copy case: {:?}",
                result.evidence
            );
        }
    }
}

#[test]
fn independent_deleted_common_prefix_cannot_be_stitched_to_the_next_paragraph() {
    for cluster in CLUSTERS {
        let common = format!(
            "共享前缀{cluster}{}",
            "这些相同文字跨越保存的局部上下文。".repeat(50)
        );
        let parts = (0..45)
            .map(|i| format!("{common}本段末尾的独立编号={i:04}。\n\n"))
            .collect::<Vec<_>>();
        let old = parts.concat();
        let selected = 22;
        let old_byte = parts[..selected].iter().map(String::len).sum::<usize>()
            + parts[selected].find(cluster).unwrap();
        let anchor = reading_precision::capture(&old, old_byte);
        let new = parts
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != selected)
            .map(|(_, text)| text.as_str())
            .collect::<String>();
        // The former left paragraph and the next common prefix now meet.
        // A unique full context can therefore still point at the WRONG text.
        let result = reading_precision::resolve(&new, &anchor);
        assert!(
            !result.exact,
            "Deleted paragraph false exact: {:?}",
            result.evidence
        );
    }
}

#[test]
fn independent_deleted_section_must_not_reuse_an_identical_body_elsewhere() {
    for cluster in CLUSTERS {
        let parts = (0..9).map(|i| chapter(i, cluster)).collect::<Vec<_>>();
        let old = parts.concat();
        let old_byte = parts[..4].iter().map(String::len).sum::<usize>()
            + parts[4].find(&format!("{cluster}，正文")).unwrap();
        let anchor = reading_precision::capture(&old, old_byte);
        let new = parts
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 4)
            .map(|(_, text)| text.as_str())
            .collect::<String>();
        let result = reading_precision::resolve(&new, &anchor);
        assert!(
            !result.exact,
            "Deleted section false exact: {:?}",
            result.evidence
        );
    }
}

#[test]
fn independent_one_sided_rewrite_in_repeated_bodies_uses_verified_section_correspondence() {
    for cluster in CLUSTERS {
        let parts = (0..9).map(|i| chapter(i, cluster)).collect::<Vec<_>>();
        let source = parts.concat();
        let old_byte = parts[..4].iter().map(String::len).sum::<usize>()
            + parts[4].find(&format!("{cluster}，正文")).unwrap();
        let anchor = reading_precision::capture(&source, old_byte);
        for left_side in [true, false] {
            let (start, end) = if left_side {
                let before = &source[..old_byte];
                (
                    before.char_indices().rev().nth(35).unwrap().0,
                    before.char_indices().rev().nth(7).unwrap().0,
                )
            } else {
                let at = old_byte + cluster.len();
                let after = &source[at..];
                (
                    at + after.char_indices().nth(8).unwrap().0,
                    at + after.char_indices().nth(36).unwrap().0,
                )
            };
            let inserted = "修订后的说明保留真实阅读文字。";
            let new = format!("{}{}{}", &source[..start], inserted, &source[end..]);
            let expected = if left_side {
                old_byte - (end - start) + inserted.len()
            } else {
                old_byte
            };
            let result = reading_precision::resolve(&new, &anchor);
            assert!(
                result.exact,
                "{cluster} left={left_side}: {:?}",
                result.evidence
            );
            assert_eq!(result.tier, "text");
            assert_eq!(result.source_byte, expected);
            assert_eq!(result.grapheme_range, expected..expected + cluster.len());
        }
    }
}

#[test]
fn independent_split_tail_and_merge_previous_repeated_body_never_claims_wrong_copy() {
    for cluster in CLUSTERS {
        let unit = format!(
            "中文🙂 e\u{301} 👩🏽‍💻 **强调** [链接](https://example.invalid/long-target) \\*转义\\* \
             reading anchors retain text identity across layout changes. {cluster} protected tail. "
        );
        let parts = (0..18)
            .map(|n| format!("段落 {n:05} source-{n:05}：{}\n\n", unit.repeat(3)))
            .collect::<Vec<_>>();
        let source = format!("# 独立合并拆分验收\n\n{}", parts.concat());
        let target_start = source.find("段落 00009 source-00009").unwrap();
        let target_end = target_start + parts[9].len();
        let within = source[target_start..target_end]
            .rfind(&format!("{cluster} protected tail"))
            .unwrap();
        let old_byte = target_start + within;
        let anchor = reading_precision::capture(&source, old_byte);
        let left = source[..old_byte].char_indices().rev().nth(23).unwrap().0;
        let right = old_byte
            + cluster.len()
            + source[old_byte + cluster.len()..]
                .char_indices()
                .nth(24)
                .unwrap()
                .0;
        let mut new = source.clone();
        let mut expected = old_byte;
        // Descending independently constructed splices preserve scalar identity.
        for at in [right, left] {
            new.insert_str(at, "\n\n");
            if at <= expected {
                expected += 2;
            }
        }
        let before_limit = source[..old_byte].char_indices().rev().nth(49).unwrap().0;
        let merge = new[..before_limit].rfind("\n\n").unwrap();
        new.replace_range(merge..merge + 2, " ");
        if merge < expected {
            expected -= 1;
        }
        assert_eq!(&new[expected..expected + cluster.len()], cluster);
        let result = reading_precision::resolve(&new, &anchor);
        assert!(
            !result.exact || result.grapheme_range == (expected..expected + cluster.len()),
            "False exact for independently surviving split-tail {cluster}: expected {expected}, got {:?}",
            result
        );
        assert!(
            result.exact,
            "The independently unique surviving paragraph witnesses should also recover {cluster}: {:?}",
            result.evidence
        );
    }
}

#[test]
fn independent_three_repeats_with_both_neighbor_nodes_changed_use_original_scope() {
    for cluster in CLUSTERS {
        let parts = (0..18)
            .map(|n| {
                let body = format!(
                    "Original paragraph identity {n:04}: 中文组合 e\u{301} {cluster} \
                     original information record-{n:04} and explanatory source content. "
                );
                format!(
                    "## Distinct original section {n:04}\n\n{}\n\n",
                    body.repeat(3)
                )
            })
            .collect::<Vec<_>>();
        let source = parts.concat();
        let part_start = parts[..9].iter().map(String::len).sum::<usize>();
        let old_byte = part_start + parts[9].rfind(&format!("{cluster} original")).unwrap();
        let anchor = reading_precision::capture(&source, old_byte);
        let left = source[..old_byte].char_indices().rev().nth(23).unwrap().0;
        let old_end = old_byte + source[old_byte..].find("\n\n").unwrap();
        let right = old_end + source[old_end..].char_indices().nth(6).unwrap().0;
        let merge = source[..old_byte].rfind("\n\n").unwrap();
        let edits = [
            (right, right, "\n\n"),
            (left, left, "\n\n"),
            (merge, merge + 2, " "),
        ];
        let mut new = source.clone();
        let mut expected = old_byte;
        for (start, end, value) in edits {
            new.replace_range(start..end, value);
            if start <= expected {
                expected = expected + value.len() - (end - start);
            }
        }
        assert_eq!(&new[expected..expected + cluster.len()], cluster);
        let result = reading_precision::resolve(&new, &anchor);
        assert!(
            result.exact,
            "Both-neighbor scope for {cluster}: {:?}",
            result.evidence
        );
        assert_eq!(result.tier, "text");
        assert_eq!(result.grapheme_range, expected..expected + cluster.len());

        // Deleting the complete target body leaves the same unique titles;
        // they establish a scope, and cannot invent a surviving target.
        let body_start = part_start + parts[9].find("\n\n").unwrap() + 2;
        let deleted = format!("{}{}", &source[..body_start], &source[old_end..]);
        let missing = reading_precision::resolve(&deleted, &anchor);
        assert!(
            !missing.exact,
            "Deleted three-repeat body for {cluster}: {:?}",
            missing.evidence
        );
    }
}

#[test]
fn independent_paragraph_and_single_newline_text_keep_each_repeated_interior_target() {
    for cluster in CLUSTERS {
        for separator in ["\n", "\n\n"] {
            for repeated_identity in [false, true] {
                let body = format!(
                    "unchanging repeated opening words and visible inline **source** [label](url) \
                 独立目标{cluster}位置 follows repeated body text with a sufficiently long immutable ending. "
                );
                let rows = (0..24)
                    .map(|n| {
                        let identity = format!("逻辑内容单元 {n:05} unique-unit-{n:05}: ");
                        let content = if repeated_identity {
                            format!("{identity}{body}").repeat(3)
                        } else {
                            format!("{identity}{}", body.repeat(3))
                        };
                        format!("{content}{separator}")
                    })
                    .collect::<Vec<_>>();
                let source = rows.concat();
                let row_start = rows[..12].iter().map(String::len).sum::<usize>();
                for repeated_copy in 0..3 {
                    let byte_in_row = rows[12]
                        .match_indices(&format!("独立目标{cluster}位置"))
                        .nth(repeated_copy)
                        .unwrap()
                        .0
                        + "独立目标".len();
                    let old_byte = row_start + byte_in_row;
                    let anchor = reading_precision::capture(&source, old_byte);
                    let left = source[..old_byte].char_indices().rev().nth(23).unwrap().0;
                    let after_cluster = old_byte + cluster.len();
                    let right =
                        after_cluster + source[after_cluster..].char_indices().nth(24).unwrap().0;
                    let merge = row_start - separator.len();
                    let mut new = source.clone();
                    new.insert_str(right, "\n\n");
                    new.insert_str(left, "\n\n");
                    new.replace_range(merge..row_start, " ");
                    let expected = old_byte + 2 + 1 - separator.len();
                    assert_eq!(&new[expected..expected + cluster.len()], cluster);
                    let result = reading_precision::resolve(&new, &anchor);
                    assert!(
                        result.exact,
                        "Surviving interior copy={repeated_copy}, repeated_identity={repeated_identity}, separator={separator:?}, {cluster}: {:?}",
                        result.evidence
                    );
                    assert_eq!(result.tier, "text");
                    assert_eq!(result.grapheme_range, expected..expected + cluster.len());
                    // The same common body survives in every other row. Removing
                    // the original complete row must not invent correspondence.
                    let row_end = row_start + rows[12].len();
                    let removed = format!("{}{}", &source[..row_start], &source[row_end..]);
                    let missing = reading_precision::resolve(&removed, &anchor);
                    assert!(
                        !missing.exact,
                        "Deleted interior copy={repeated_copy}, separator={separator:?}, {cluster}: {:?}",
                        missing.evidence
                    );
                }
            }
        }
    }
}
