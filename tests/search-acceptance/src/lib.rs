#![allow(dead_code)]
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
extern crate self as eframe;
pub use egui;
#[path = "../../../src/reading_state.rs"]
mod reading_state;
#[path = "../../../src/reading_precision.rs"]
mod reading_precision;
#[path = "../../../src/reading_geometry.rs"]
mod reading_geometry;
#[path = "../../../src/document.rs"]
mod document;
#[path = "../../../src/encoding.rs"]
mod encoding;
#[path = "../../../src/localization.rs"]
mod localization;
#[path = "../../../src/search.rs"]
mod search;
#[path = "../../../src/text_files.rs"]
mod text_files;

#[cfg(test)]
mod independent {
    use super::{document::Document, encoding::Encoding, search::*};
    use std::{
        fs,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64},
        },
        time::{Duration, Instant},
    };

    fn guard() -> Guard {
        Guard::new(Arc::new(AtomicU64::new(1)), 1, Duration::from_secs(5))
    }
    fn options(query: &str, replacement: &str, regex: bool) -> Options {
        Options {
            query: query.into(),
            replacement: replacement.into(),
            regex,
            ..Default::default()
        }
    }
    fn index(root: &Path) -> Index {
        let mut i = Index::new(root).unwrap();
        i.refresh(true, &AtomicBool::new(false));
        i
    }
    fn replaced(text: &str, o: Options) -> String {
        replace_selected(
            text,
            &Matcher::new(&o).unwrap().edits(text, &guard()).unwrap(),
        )
    }

    #[test]
    fn followup_cached_search_keeps_snapshot_stamp_unicode_lines_and_live_overlay_exact() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("稿件 🦀.md");
        let initial = "第一行\r\n猫 needle\r\n\r\n末行 needle";
        fs::write(&path, initial).unwrap();
        let mut idx = index(d.path());
        let original = fs::read(&path).unwrap();
        let check = |idx: &Index, overlays: &[Document], text: &str, lines: &[usize]| {
            let found = idx
                .search(&options("needle", "", false), overlays, false, &guard())
                .unwrap();
            assert_eq!(found.count, lines.len());
            let hits = &found.groups[0].hits;
            assert_eq!(hits.iter().map(|h| h.line).collect::<Vec<_>>(), lines);
            assert_eq!(
                hits.iter().map(|h| h.start).collect::<Vec<_>>(),
                text.to_ascii_lowercase()
                    .match_indices("needle")
                    .map(|(n, _)| n)
                    .collect::<Vec<_>>()
            );
            for h in hits {
                assert!(text[h.start..h.end].eq_ignore_ascii_case("needle"));
                assert_eq!(h.stamp, text_stamp(text));
                assert!(h.snippet[h.highlight[0]..h.highlight[1]].eq_ignore_ascii_case("needle"));
            }
        };
        // Warmed immutable snapshots must remain correct across distinct queries.
        for _ in 0..3 {
            check(&idx, &[], initial, &[2, 4]);
            assert_eq!(
                idx.search(&options("missing", "", false), &[], false, &guard())
                    .unwrap()
                    .count,
                0
            );
        }
        let mut overlay = Document::open(&path).unwrap();
        overlay.text = "needle\n中\n🦀 needle\nneedle".into();
        check(&idx, &[overlay.clone()], &overlay.text, &[1, 3, 4]);
        overlay.text = "一\n\nneedle".into();
        check(&idx, &[overlay.clone()], &overlay.text, &[3]);
        check(&idx, &[], initial, &[2, 4]);
        assert_eq!(fs::read(&path).unwrap(), original);
        let mtime = fs::metadata(&path).unwrap().modified().unwrap();
        let changed = initial.replacen("needle", "NEEDLE", 1);
        fs::write(&path, &changed).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        idx.refresh(true, &AtomicBool::new(false));
        check(&idx, &[], &changed, &[2, 4]);
        let exact = idx
            .search(
                &Options {
                    case: true,
                    ..options("needle", "", false)
                },
                &[],
                false,
                &guard(),
            )
            .unwrap();
        assert_eq!(exact.count, 1);
        assert_eq!(exact.groups[0].hits[0].line, 4);
        // EOF and multiline/zero-width matches must retain byte and line semantics.
        let eof = idx
            .search(&options("$", "", true), &[], false, &guard())
            .unwrap();
        assert_eq!(eof.groups[0].hits[0].start, changed.len());
        assert_eq!(eof.groups[0].hits[0].line, 4);
    }

    #[test]
    fn capture_groups_literals_and_unicode_keep_exact_newlines() {
        assert_eq!(
            replaced(
                "猫 12\r\n犬 34\r\n",
                options(r"(?m)^(?<animal>\S+) (\d+)\r?$", "$2:$<animal>\r", true)
            ),
            "12:猫\r\n34:犬\r\n"
        );
        assert_eq!(
            replaced("a a", options("a", "$1 $<name> $$", false)),
            "$1 $<name> $$ $1 $<name> $$"
        );
        assert_eq!(
            replaced("a", options("(a)", "$1/$$/$<missing>", true)),
            "a/$/"
        );
        assert_eq!(
            replaced(
                "ab",
                options("(a)(?<tail>b)", "$1suffix-$2end-$<tail>next", true)
            ),
            "asuffix-bend-bnext"
        );
        let mut o = options("cat", "X", false);
        o.word = true;
        assert_eq!(
            replaced("cat cats cat_ _cat cat猫 cat\u{301} cat!", o),
            "X cats cat_ _cat cat猫 cat\u{301} X!"
        );
        assert_eq!(replaced("猫🦀", options("(?m)^|$", "!", true)), "!猫🦀!");
    }

    #[test]
    fn user_can_choose_one_hit_and_one_file_without_other_changes() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("a.md"), "one one\r\n").unwrap();
        fs::write(d.path().join("b.md"), "one").unwrap();
        let i = index(d.path());
        let mut p = i
            .preview(&options("one", "two", false), &[], false, &guard())
            .unwrap();
        p.files[0].edits[1].selected = false;
        for e in &mut p.files[1].edits {
            e.selected = false;
        }
        let (_, id) = commit(&p).unwrap();
        assert_eq!(
            fs::read_to_string(d.path().join("a.md")).unwrap(),
            "two one\r\n"
        );
        assert_eq!(fs::read_to_string(d.path().join("b.md")).unwrap(), "one");
        rollback(d.path(), &id, &[]).unwrap();
        assert_eq!(
            fs::read_to_string(d.path().join("a.md")).unwrap(),
            "one one\r\n"
        );
    }

    #[test]
    fn filters_apply_to_root_and_nested_files_and_report_invalid_glob() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("notes")).unwrap();
        for f in ["a.md", "notes/a.md", "notes/skip.md", "notes/a.txt"] {
            fs::write(d.path().join(f), "target").unwrap();
        }
        let i = index(d.path());
        let mut o = options("target", "", false);
        o.include = "**/*.md".into();
        o.exclude = "**/skip.md".into();
        assert_eq!(i.search(&o, &[], false, &guard()).unwrap().count, 2);
        o.include = "[".into();
        assert!(Matcher::new(&o).is_err());
    }

    #[test]
    fn single_document_ignores_hidden_workspace_path_filters() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.md");
        fs::write(&path, "target").unwrap();
        let i = index(d.path());
        let doc = Document::open(&path).unwrap();
        let mut o = options("target", "changed", false);
        o.include = "notes/**".into();
        o.exclude = "**/*.md".into();
        assert_eq!(
            i.search(&o, &[doc.clone()], false, &guard()).unwrap().count,
            0
        );
        assert_eq!(
            i.search(&o, &[doc.clone()], true, &guard()).unwrap().count,
            1
        );
        let p = i.preview(&o, &[doc], true, &guard()).unwrap();
        assert_eq!(p.files.len(), 1);
        assert_eq!(p.files[0].after(), "changed");
        o.include = "[".into();
        let doc = Document::open(&path).unwrap();
        assert_eq!(
            i.search(&o, &[doc.clone()], true, &guard()).unwrap().count,
            1
        );
        assert_eq!(
            i.preview(&o, &[doc], true, &guard()).unwrap().files.len(),
            1
        );
    }

    #[test]
    fn dirty_overlay_replaces_disk_content_in_results_and_preview() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.md");
        fs::write(&path, "disk-only").unwrap();
        let i = index(d.path());
        let mut doc = Document::open(&path).unwrap();
        doc.text = "memory-only".into();
        assert_eq!(
            i.search(
                &options("disk-only", "", false),
                &[doc.clone()],
                false,
                &guard()
            )
            .unwrap()
            .count,
            0
        );
        let p = i
            .preview(
                &options("memory-only", "changed", false),
                &[doc],
                false,
                &guard(),
            )
            .unwrap();
        assert_eq!(p.files.len(), 1);
        commit(&p).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "changed");
    }

    #[test]
    fn external_change_aborts_entire_preview_before_first_write() {
        let d = tempfile::tempdir().unwrap();
        for f in ["a.md", "z.md"] {
            fs::write(d.path().join(f), "one").unwrap();
        }
        let p = index(d.path())
            .preview(&options("one", "two", false), &[], false, &guard())
            .unwrap();
        fs::write(d.path().join("z.md"), "external").unwrap();
        assert!(commit(&p).is_err());
        assert_eq!(fs::read_to_string(d.path().join("a.md")).unwrap(), "one");
        assert_eq!(
            fs::read_to_string(d.path().join("z.md")).unwrap(),
            "external"
        );
    }

    #[test]
    fn same_file_path_alias_cannot_bypass_external_change_guard() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("sub")).unwrap();
        let path = d.path().join("a.md");
        fs::write(&path, "one").unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.text = "edited".into();
        fs::write(&path, "external").unwrap();
        assert!(doc.save_to(&d.path().join("sub/../a.md")).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "external");
        assert!(doc.dirty());
    }

    #[test]
    fn pending_encoding_conversion_and_dirty_source_survive_rollback() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.md");
        fs::write(&path, "one 猫\r\n").unwrap();
        let i = index(d.path());
        let mut doc = Document::open(&path).unwrap();
        doc.text.push_str("dirty one");
        doc.set_encoding(Encoding::Utf16Le, true).unwrap();
        let p = i
            .preview(
                &options("one", "two", false),
                &[doc.clone()],
                false,
                &guard(),
            )
            .unwrap();
        let (_, id) = commit(&p).unwrap();
        assert!(fs::read(&path).unwrap().starts_with(&[0xff, 0xfe]));
        let restored = rollback(d.path(), &id, &[]).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"one \xe7\x8c\xab\r\n");
        assert_eq!(restored[0].text, doc.text);
        assert_eq!(restored[0].encoding, Encoding::Utf16Le);
        assert!(restored[0].bom);
        assert!(restored[0].dirty());
    }

    #[test]
    fn rollback_dirty_guard_is_all_or_nothing_and_keeps_external_updates() {
        let d = tempfile::tempdir().unwrap();
        for f in ["a.md", "b.md"] {
            fs::write(d.path().join(f), "one").unwrap();
        }
        let p = index(d.path())
            .preview(&options("one", "two", false), &[], false, &guard())
            .unwrap();
        let (_, id) = commit(&p).unwrap();
        let mut doc = Document::open(&d.path().join("b.md")).unwrap();
        doc.text.push_str(" new edit");
        assert!(rollback(d.path(), &id, &[doc]).is_err());
        assert_eq!(fs::read_to_string(d.path().join("a.md")).unwrap(), "two");
        fs::write(d.path().join("b.md"), "external").unwrap();
        assert!(rollback(d.path(), &id, &[]).is_err());
        assert_eq!(
            fs::read_to_string(d.path().join("b.md")).unwrap(),
            "external"
        );
    }

    #[test]
    fn invalid_snapshot_metadata_fails_before_any_rollback_write() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("a.md");
        fs::write(&path, "one").unwrap();
        let p = index(d.path())
            .preview(&options("one", "two", false), &[], false, &guard())
            .unwrap();
        let (_, id) = commit(&p).unwrap();
        let manifest = d
            .path()
            .join(".mdcard-history")
            .join(&id)
            .join("manifest.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        value["files"][0]["encoding"] = "Unsupported-Corrupt-Codec".into();
        fs::write(manifest, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(rollback(d.path(), &id, &[]).is_err());
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "two",
            "validation must precede rollback writes"
        );
    }

    #[test]
    fn title_alias_fuzzy_and_mru_ranking_have_observable_effect() {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("aa.md"),
            "---\naliases:\n  - 独特别名\n---\n# Unique Heading\n",
        )
        .unwrap();
        fs::write(d.path().join("ab.md"), "# Other\n").unwrap();
        let i = index(d.path());
        assert_eq!(i.quick("独特", &[], &guard()).unwrap().len(), 1);
        assert_eq!(i.quick("uqhd", &[], &guard()).unwrap()[0].label, "aa.md");
        let ab = fs::canonicalize(d.path().join("ab.md")).unwrap();
        assert_eq!(i.quick("a", &[ab.clone()], &guard()).unwrap()[0].path, ab);
    }

    #[test]
    fn line_number_and_highlight_offsets_map_unicode_multiline_exactly() {
        let text = "前缀🦀\r\n第2行\r\n猫 target\r\nlast";
        let start = text.find("target").unwrap();
        let h = hit(text, start, start + 6);
        assert_eq!(h.line, 3);
        assert_eq!(&h.snippet[h.highlight[0]..h.highlight[1]], "target");
        assert_eq!(&text[h.start..h.end], "target");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_ancestors_and_history_paths_cannot_escape_workspace() {
        let d = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        fs::write(out.path().join("a.md"), "outside").unwrap();
        std::os::unix::fs::symlink(out.path(), d.path().join("linked")).unwrap();
        assert!(confined(d.path(), Path::new("linked/a.md"), true).is_err());
        assert!(confined(d.path(), Path::new("../a.md"), false).is_err());
        assert_eq!(index(d.path()).status.files, 0);
    }

    #[test]
    fn worker_latest_generation_and_incremental_create_delete_are_real() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("a.md"), "first").unwrap();
        let w = Worker::start(d.path().to_owned(), Arc::new(|| {}));
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(r) = w.poll() {
                if matches!(r.output, Output::Status(_)) {
                    break;
                }
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        w.send(Command::Query {
            options: options("first", "", false),
            overlays: vec![],
            single: false,
        });
        let last = w.send(Command::Query {
            options: options("second", "", false),
            overlays: vec![],
            single: false,
        });
        loop {
            if let Some(r) = w.poll() {
                if r.generation == last {
                    if let Output::Results(v) = r.output {
                        assert_eq!(v.count, 0);
                        break;
                    } else {
                        panic!("latest worker query failed");
                    }
                }
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        fs::write(d.path().join("b.md"), "second").unwrap();
        fs::remove_file(d.path().join("a.md")).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        let id = w.send(Command::Query {
            options: options("second", "", false),
            overlays: vec![],
            single: false,
        });
        loop {
            if let Some(r) = w.poll() {
                if r.generation == id {
                    if let Output::Results(v) = r.output {
                        assert_eq!(v.count, 1);
                        assert!(v.groups[0].relative.ends_with("b.md"));
                        break;
                    } else {
                        panic!("incremental query failed");
                    }
                }
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    #[ignore = "explicit release-mode end-to-end timing run"]
    fn performance_1600_documents_across_refresh_cycles() {
        let d = tempfile::tempdir().unwrap();
        let padding = "payload a reasonable markdown paragraph with ordinary words.\n".repeat(190);
        let mut bytes = 0;
        for n in 0..1600 {
            let source = format!(
                "---\naliases: [note-{n}]\n---\n# Document {n}\nentry-{n} 中文🦀\nneedle\n{padding}"
            );
            bytes += source.len();
            fs::write(d.path().join(format!("note-{n:04}.md")), source).unwrap();
        }
        let started = Instant::now();
        let w = Worker::start(d.path().to_owned(), Arc::new(|| {}));
        let until = Instant::now() + Duration::from_secs(90);
        loop {
            if let Some(r) = w.poll() {
                if let Output::Status(s) = r.output {
                    assert_eq!(s.files, 1600);
                    break;
                }
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
        let index_ms = started.elapsed().as_secs_f64() * 1000.;
        let mut rounds = Vec::new();
        for n in 0..160 {
            if n == 50 {
                fs::write(
                    d.path().join("note-0000.md"),
                    "# Updated\nentry-0 中文🦀\nneedle\n",
                )
                .unwrap();
            }
            let (q, re, count) = match n % 4 {
                0 => ("needle", false, 1600),
                1 => ("中文🦀", false, 1600),
                2 => ("unfindable-rare-token-987", false, 0),
                _ => (r"(?m)^entry-(\d+) 中文🦀$", true, 1600),
            };
            let started = Instant::now();
            let id = w.send(Command::Query {
                options: options(q, "", re),
                overlays: vec![],
                single: false,
            });
            loop {
                if let Some(r) = w.poll() {
                    if r.generation == id {
                        if let Output::Results(v) = r.output {
                            assert_eq!(v.count, count);
                            rounds.push((
                                started.elapsed().as_secs_f64() * 1000.,
                                r.roundtrip_ms,
                                v.elapsed_ms,
                            ));
                            break;
                        } else if let Output::Error(e) = r.output {
                            panic!("query {q}: {e}");
                        } else {
                            panic!("unexpected query output");
                        }
                    }
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_micros(100));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut wall: Vec<f64> = rounds.iter().map(|r| r.0).collect();
        wall.sort_by(f64::total_cmp);
        let report = serde_json::json!({"corpus_documents":1600,"corpus_bytes":bytes,"initial_index_ms":index_ms,"samples":rounds.len(),"duration_ms":started.elapsed().as_secs_f64()*1000.,"wall_p50_ms":wall[80],"wall_p95_ms":wall[152],"wall_p99_ms":wall[158],"wall_max_ms":wall[159],"rounds":rounds});
        println!("INDEPENDENT_SEARCH_PERFORMANCE={report}");
        if let Ok(output) = std::env::var("MDCARD_INDEPENDENT_SEARCH_PERF") {
            fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
        assert!(
            wall[159] < 50.,
            "worker response including refresh queues exceeded 50ms: {}",
            wall[159]
        );
    }

    #[test]
    #[ignore = "explicit release-mode adversarial deadline run"]
    fn adversarial_single_match_deadline_is_effective() {
        let text = "a".repeat(16 * 1024 * 1024 - 1) + "!";
        let patterns = [
            r"(a+)+$",
            r"(a|aa)+$",
            r"(?:a?){500}a{500}$",
            r"(?:\w+\s?){20}$",
            r"(?s:.*a){50}$",
            r"(a|aa)+",
            r"(?:a?){500}a{500}",
            r"(?s:.*a){50}",
        ];
        for p in patterns {
            let started = Instant::now();
            let matcher = match Matcher::new(&options(p, "", true)) {
                Ok(m) => m,
                Err(e) => {
                    println!("REJECTED {p}: {e}");
                    continue;
                }
            };
            let g = Guard::new(Arc::new(AtomicU64::new(1)), 1, Duration::from_millis(10));
            let outcome = matcher.edits(&text, &g);
            let elapsed = started.elapsed().as_secs_f64() * 1000.;
            println!(
                "ADVERSARIAL {p}: elapsed_ms={elapsed:.3}, outcome={:?}",
                outcome.as_ref().map(|v| v.len())
            );
            assert!(
                elapsed < 300.,
                "single accepted regex bypassed the 10ms deadline for >300ms: {p}: {elapsed:.3}ms"
            );
        }
    }
}
