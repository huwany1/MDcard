// SPDX-License-Identifier: AGPL-3.0-only

use crate::egui;
use crate::split_scroll::*;
use eframe::App;
#[test]
fn line_wheel_180ms_takeovers_do_not_accumulate_top_code_marker_drift() {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(1.25);
    let marker = "ANCHOR_0250";
    let text = format!(
        "# Code\n\n```text\n{}```\n",
        (0..400)
            .map(|i| format!("let ANCHOR_{i:04} = {i};\n"))
            .collect::<String>()
    );
    let byte = text.find(marker).unwrap();
    let mut app = crate::preview_layout_tests::editor(&ctx, &text);
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..8 {
        render(&mut app, &ctx, &mut frame, vec![]);
    }
    for driver in [Pane::Source, Pane::Preview] {
        app.large.split_scroll.driver = Some(driver);
        app.large.split_scroll.anchor = Some(Anchor {
            byte,
            fraction: 0.,
            viewport_y: 4.,
            text: true,
        });
        for _ in 0..8 {
            render(&mut app, &ctx, &mut frame, vec![]);
        }
    }
    let mark = |s: &SplitScroll, pane: Pane| {
        let view = &s.views[pane.index()];
        view.galleys
            .iter()
            .find_map(|g| {
                let at = g.galley.text().find(marker)?;
                let rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                    g.galley.text()[..at].chars().count(),
                ));
                Some((g.pos.y + rect.top() - view.rect?.top(), rect.height()))
            })
            .unwrap()
    };
    let initial = [
        mark(&app.large.split_scroll, Pane::Source),
        mark(&app.large.split_scroll, Pane::Preview),
    ];
    for cycle in 0..20 {
        let amount = if cycle < 10 { 0.1 } else { 1. };
        for pane in [Pane::Source, Pane::Preview] {
            for delta in [-amount, amount] {
                let pos = app.large.split_scroll.views[pane.index()]
                    .rect
                    .unwrap()
                    .center();
                render(
                    &mut app,
                    &ctx,
                    &mut frame,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Line,
                            delta: egui::vec2(0., delta),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                for _ in 0..10 {
                    render(&mut app, &ctx, &mut frame, vec![]);
                }
            }
        }
        for _ in 0..15 {
            render(&mut app, &ctx, &mut frame, vec![]);
        }
        let sync = &app.large.split_scroll;
        let actual = [mark(sync, Pane::Source), mark(sync, Pane::Preview)];
        eprintln!(
            "cycle={cycle} original={initial:?} actual={actual:?} anchor={:?} offsets=[{},{}] residual={:?}",
            sync.anchor,
            sync.views[0].offset,
            sync.views[1].offset,
            sync.anchor.map(|a| [
                sync.precise(Pane::Source, a, &app.doc.text, frame)
                    .map(|v| v - sync.views[0].offset),
                sync.precise(Pane::Preview, a, &app.doc.text, frame)
                    .map(|v| v - sync.views[1].offset)
            ])
        );
        for pane in [Pane::Source, Pane::Preview] {
            assert!(
                (actual[pane.index()].0 - initial[pane.index()].0).abs() <= actual[pane.index()].1,
                "Line wheel drift cycle {cycle} {pane:?}: {actual:?}"
            );
        }
    }
}
#[test]
fn preview_confirmed_drawn_scroll_updates_source_without_predicting_native_future() {
    let ctx = egui::Context::default();
    let mut app = crate::preview_layout_tests::editor(
        &ctx,
        &"Paragraph **mapped** content 中文.\n\n".repeat(300),
    );
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..8 {
        render(&mut app, &ctx, &mut frame, vec![]);
    }
    wheel(&mut app, &ctx, &mut frame, Pane::Source, -400.);
    let pos = app.large.split_scroll.views[1].rect.unwrap().center();
    let before = app.large.split_scroll.views[0].offset;
    let id = egui::Id::new(("source-editor", app.document_generation));
    let selection = egui::text_edit::TextEditState::load(&ctx, id)
        .unwrap()
        .cursor
        .char_range();
    let focus = ctx.memory(|m| m.focused());
    render(
        &mut app,
        &ctx,
        &mut frame,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -40.),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(
        (app.large.split_scroll.views[0].offset - before).abs() <= 1.,
        "the event pass must not project the old preview galley into its future native offset"
    );
    let latest_sequence = app.large.split_scroll.sequence;
    render(&mut app, &ctx, &mut frame, vec![]);
    assert_eq!(
        app.large.split_scroll.sequence, latest_sequence,
        "programmatic following cannot emit another user intent"
    );
    assert!(
        (app.large.split_scroll.views[0].offset - before).abs() > 1.,
        "source follows the latest preview viewport after it was actually laid out"
    );
    assert_eq!(
        egui::text_edit::TextEditState::load(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        selection
    );
    assert_eq!(ctx.memory(|m| m.focused()), focus);
    let byte = app.large.split_scroll.anchor.unwrap().byte;
    let character = app.doc.text[..byte].chars().count();
    let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(character),
        )));
    state.store(&ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
    let revision = app.doc.revision();
    render(
        &mut app,
        &ctx,
        &mut frame,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -40.),
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Paste("INSERTED".into()),
        ],
    );
    assert!(app.doc.revision() > revision);
    assert!(!app.large.split_scroll.current);
    assert!(app.large.split_scroll.anchor.is_none());
}
#[test]
fn preview_first_never_precedes_source_edits_navigation_or_stale_mapping() {
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    for event in [
        egui::Event::Text("a".into()),
        egui::Event::Paste("paste".into()),
        egui::Event::Cut,
        egui::Event::Ime(egui::ImeEvent::Preedit("中".into())),
        key(egui::Key::Z),
        key(egui::Key::Y),
        key(egui::Key::Delete),
        key(egui::Key::Backspace),
        key(egui::Key::Enter),
        key(egui::Key::ArrowDown),
        key(egui::Key::Home),
    ] {
        let ctx = egui::Context::default();
        let mut sync = SplitScroll::default();
        sync.session(Some((1, 1)), 0, true, 1, false);
        sync.intent = Some(Pane::Preview);
        let _ = ctx.run(
            egui::RawInput {
                events: vec![event.clone()],
                ..Default::default()
            },
            |ctx| {
                assert!(!sync.preview_first(ctx), "{event:?}");
            },
        );
    }
    let ctx = egui::Context::default();
    let mut sync = SplitScroll::default();
    sync.session(Some((1, 1)), 0, false, 1, false);
    sync.intent = Some(Pane::Preview);
    assert!(!sync.preview_first(&ctx));
}
#[test]
fn subpixel_settled_targets_never_write_another_offset() {
    let ctx = egui::Context::default();
    let mut sync = SplitScroll::default();
    sync.session(Some((1, 1)), 0, true, 1, false);
    sync.driver = Some(Pane::Preview);
    sync.anchor = Some(Anchor {
        byte: 0,
        fraction: 0.,
        viewport_y: 0.,
        text: false,
    });
    sync.views[0].offset = 200.;
    sync.views[0].max = 1000.;
    let _ = ctx.run(Default::default(), |ctx| {
        for target in [200., 200.8, 199.2, 200.4] {
            assert!(
                sync.target(Pane::Source, "", ctx, |_| Some(target))
                    .is_none()
            );
            assert_eq!(sync.views[0].offset, 200.);
        }
    });
    assert_eq!(sync.repaint_requests, 0);
}
#[test]
fn settled_following_does_not_create_navigation_or_zero_delay_repaints() {
    let ctx = egui::Context::default();
    let mut app = crate::preview_layout_tests::editor(
        &ctx,
        &"A settled paragraph **content**.\n\n".repeat(250),
    );
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..8 {
        render(&mut app, &ctx, &mut frame, vec![]);
    }
    wheel(&mut app, &ctx, &mut frame, Pane::Preview, -240.);
    std::thread::sleep(std::time::Duration::from_millis(150));
    for _ in 0..8 {
        render(&mut app, &ctx, &mut frame, vec![]);
    }
    assert!(app.large.split_scroll.following(Pane::Source));
    assert!(
        !app.large.split_scroll.operating(Pane::Source),
        "a settled follower is not navigation or user input"
    );
    let mut last = std::time::Duration::ZERO;
    let requests = app.large.split_scroll.repaint_requests;
    for _ in 0..4 {
        frame += 1;
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600., 900.),
                )),
                time: Some(frame as f64 / 60.),
                ..Default::default()
            },
            |ctx| app.update(ctx, &mut eframe::Frame::_new_kittest()),
        );
        last = out.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    }
    assert_eq!(
        app.large.split_scroll.repaint_requests, requests,
        "sync caused an idle repaint; unrelated app delay {last:?}"
    );
}
#[test]
fn preview_page_command_is_consumed_before_focused_source_can_move_its_caret() {
    let ctx = egui::Context::default();
    let mut app = crate::preview_layout_tests::editor(
        &ctx,
        &"A paragraph of mapped content.\n\n".repeat(300),
    );
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..8 {
        render(&mut app, &ctx, &mut frame, vec![]);
    }
    let id = egui::Id::new(("source-editor", app.document_generation));
    ctx.memory_mut(|m| m.request_focus(id));
    wheel(&mut app, &ctx, &mut frame, Pane::Preview, -150.);
    let before = egui::text_edit::TextEditState::load(&ctx, id)
        .unwrap()
        .cursor
        .char_range();
    let offset = app.large.split_scroll.views[1].offset;
    render(
        &mut app,
        &ctx,
        &mut frame,
        vec![egui::Event::Key {
            key: egui::Key::PageDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(app.large.split_scroll.views[1].offset > offset + 100.);
    assert_eq!(app.large.split_scroll.driver, Some(Pane::Preview));
    assert_eq!(
        egui::text_edit::TextEditState::load(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        before
    );
    render(
        &mut app,
        &ctx,
        &mut frame,
        vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(
        app.large.split_scroll.driver.is_none(),
        "cursor navigation yields to native reveal without reverse scroll input"
    );
}
fn render(app: &mut crate::Editor, ctx: &egui::Context, frame: &mut u64, events: Vec<egui::Event>) {
    *frame += 1;
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600., 900.),
            )),
            time: Some(*frame as f64 / 60.),
            events,
            ..Default::default()
        },
        |ctx| app.update(ctx, &mut eframe::Frame::_new_kittest()),
    );
}
fn wheel(app: &mut crate::Editor, ctx: &egui::Context, frame: &mut u64, pane: Pane, delta: f32) {
    let pos = app.large.split_scroll.views[pane.index()]
        .rect
        .expect("mounted viewport")
        .center();
    render(
        app,
        ctx,
        frame,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., delta),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..22 {
        render(app, ctx, frame, vec![]);
    }
}
#[test]
fn native_bidirectional_content_anchor_and_takeovers_keep_source_unchanged() {
    for size in [8192, 65535, 65537, 1048575, 1048577, 16777216] {
        let ctx = egui::Context::default();
        let row = "Paragraph 000000 **label** 中文 and a repeatable content anchor.\n\n";
        let mut text = (0..size / row.len())
            .map(|i| {
                format!("Paragraph {i:06} **label** 中文 and a repeatable content anchor.\n\n")
            })
            .collect::<String>();
        text.push_str(&"x".repeat(size - text.len()));
        assert_eq!(text.len(), size);
        let mut app = crate::preview_layout_tests::editor(&ctx, "");
        app.doc = crate::document::Document::from_text(&text);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            app.large.ensure(&text, 0, app.doc.revision(), &ctx);
            if !app.large.active || app.large.parsed.is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "large fixture parser timeout {size}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..8 {
            render(&mut app, &ctx, &mut frame, vec![]);
        }
        assert_eq!(app.large.active, size >= 65536, "actual source editor path");
        let before = app.doc.text.clone();
        let revision = app.doc.revision();
        let dirty = app.doc.dirty();
        wheel(&mut app, &ctx, &mut frame, Pane::Source, -240.);
        assert_eq!(app.large.split_scroll.driver, Some(Pane::Source));
        assert!(
            app.large.split_scroll.views[1].offset > 10.,
            "ordinary and virtual preview must follow {size}"
        );
        wheel(&mut app, &ctx, &mut frame, Pane::Preview, -150.);
        assert_eq!(app.large.split_scroll.driver, Some(Pane::Preview));
        for _ in 0..2 {
            wheel(&mut app, &ctx, &mut frame, Pane::Source, -12.);
            wheel(&mut app, &ctx, &mut frame, Pane::Preview, 12.);
        }
        assert_eq!(app.doc.text, before);
        assert_eq!(app.doc.revision(), revision);
        assert_eq!(app.doc.dirty(), dirty);
    }
}
#[test]
fn actual_markdown_glyph_position_survives_twenty_roundtrips_without_mapping_as_oracle() {
    for large in [false, true] {
        let ctx = egui::Context::default();
        let marker = "独立标注0030";
        let mut text = (0..100)
            .map(|i| format!("## 标题{i}\n\n**独立标注{i:04}** same repeated text. 中文长句。\n\n"))
            .collect::<String>();
        if large {
            text.push_str(&"additional prose.\n\n".repeat(5000));
        }
        let byte = text.find(marker).unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, &text);
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..8 {
            render(&mut app, &ctx, &mut frame, vec![]);
        }
        for round in 0..20 {
            for driver in [Pane::Source, Pane::Preview] {
                app.large.split_scroll.driver = Some(driver);
                app.large.split_scroll.anchor = Some(Anchor {
                    byte,
                    fraction: 0.,
                    viewport_y: 100.,
                    text: true,
                });
                for _ in 0..8 {
                    render(&mut app, &ctx, &mut frame, vec![]);
                }
                let follower = if driver == Pane::Source {
                    Pane::Preview
                } else {
                    Pane::Source
                };
                let view = &app.large.split_scroll.views[follower.index()];
                // Locate literal rendered characters in the actual native
                // Galley. No source segment/project/target function is used
                // to derive the expectation (100 logical pixels).
                let y = view
                    .galleys
                    .iter()
                    .find_map(|g| {
                        let at = g.galley.text().find(marker)?;
                        let c = g.galley.text()[..at].chars().count();
                        Some(
                            g.pos.y + g.galley.pos_from_cursor(egui::text::CCursor::new(c)).top()
                                - view.rect?.top(),
                        )
                    })
                    .expect("independently labelled rendered marker mounted");
                assert!(
                    (y - 100.).abs() <= app.font_size * 1.5,
                    "{large} round {round} {driver:?} y={y}"
                );
            }
        }
    }
}
#[test]
fn twenty_native_roundtrips_preserve_one_original_content_marker() {
    for large in [false, true] {
        let ctx = egui::Context::default();
        let marker = "独立标注0030";
        let mut text = (0..100)
            .map(|i| format!("## 标题{i}\n\n**独立标注{i:04}** same repeated text. 中文长句。\n\n"))
            .collect::<String>();
        if large {
            text.push_str(&"additional prose.\n\n".repeat(5000));
        }
        let byte = text.find(marker).unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, &text);
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..8 {
            render(&mut app, &ctx, &mut frame, vec![]);
        }
        // Only this initial setup names the desired source coordinate.
        for driver in [Pane::Source, Pane::Preview] {
            app.large.split_scroll.driver = Some(driver);
            app.large.split_scroll.anchor = Some(Anchor {
                byte,
                fraction: 0.,
                viewport_y: 100.,
                text: true,
            });
            for _ in 0..8 {
                render(&mut app, &ctx, &mut frame, vec![]);
            }
        }
        let marker_y = |s: &SplitScroll, pane: Pane| {
            let v = &s.views[pane.index()];
            v.galleys
                .iter()
                .find_map(|g| {
                    let at = g.galley.text().find(marker)?;
                    Some(
                        g.pos.y
                            + g.galley
                                .pos_from_cursor(egui::text::CCursor::new(
                                    g.galley.text()[..at].chars().count(),
                                ))
                                .top()
                            - v.rect?.top(),
                    )
                })
                .expect("literal marker remains mounted")
        };
        let initial = [
            marker_y(&app.large.split_scroll, Pane::Source),
            marker_y(&app.large.split_scroll, Pane::Preview),
        ];
        for round in 0..20 {
            let amount = if round < 10 { 4. } else { 40. };
            for pane in [Pane::Source, Pane::Preview] {
                wheel(&mut app, &ctx, &mut frame, pane, -amount);
                wheel(&mut app, &ctx, &mut frame, pane, amount);
            }
            for pane in [Pane::Source, Pane::Preview] {
                let y = marker_y(&app.large.split_scroll, pane);
                assert!(
                    (y - initial[pane.index()]).abs() <= app.font_size * 1.5,
                    "{large} round {round} {pane:?}: {y} initial {}",
                    initial[pane.index()]
                );
            }
        }
        for pane in [Pane::Source, Pane::Preview] {
            for delta in [-40., -80., 80., 40.] {
                let pos = app.large.split_scroll.views[pane.index()]
                    .rect
                    .unwrap()
                    .center();
                render(
                    &mut app,
                    &ctx,
                    &mut frame,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0., delta),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            for _ in 0..22 {
                render(&mut app, &ctx, &mut frame, vec![]);
            }
            for p in [Pane::Source, Pane::Preview] {
                let y = marker_y(&app.large.split_scroll, p);
                assert!(
                    (y - initial[p.index()]).abs() <= app.font_size * 1.5,
                    "rapid reverse {large} {p:?} {y}"
                );
            }
        }
    }
}
#[test]
fn stale_coordinates_session_reset_and_programmatic_motion_never_retain_old_targets() {
    let mut s = SplitScroll::default();
    s.session(Some((1, 2)), 3, true, 4, false);
    s.driver = Some(Pane::Source);
    s.anchor = Some(Anchor {
        byte: 500,
        fraction: 0.,
        viewport_y: 8.,
        text: true,
    });
    s.session(Some((1, 2)), 4, false, 4, false);
    assert!(s.anchor.is_none());
    assert!(!s.following(Pane::Preview));
    s.session(Some((1, 2)), 4, true, 5, false);
    assert!(s.anchor.is_none());
    s.session(Some((2, 2)), 4, true, 5, false);
    assert!(s.driver.is_none());
    assert!(s.intent.is_none());
    s.session(None, 4, true, 5, false);
    assert!(!s.enabled());
}
#[test]
fn stale_snapshot_retains_latest_physical_scroll_side_until_current_mapping_returns() {
    let ctx = egui::Context::default();
    let text = "A paragraph with **mapped** text.\n\n".repeat(200);
    let mut app = crate::preview_layout_tests::editor(&ctx, &text);
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..8 {
        render(&mut app, &ctx, &mut frame, vec![]);
    }
    app.doc.text.insert_str(0, "Inserted above viewport\n\n");
    app.doc.mark_text_changed();
    app.rich.hold_parser_for_test(true);
    render(&mut app, &ctx, &mut frame, vec![]);
    assert!(!app.large.split_scroll.current);
    wheel(&mut app, &ctx, &mut frame, Pane::Preview, -80.);
    assert_eq!(app.large.split_scroll.driver, Some(Pane::Preview));
    assert!(app.large.split_scroll.anchor.is_none());
    wheel(&mut app, &ctx, &mut frame, Pane::Source, -40.);
    assert_eq!(app.large.split_scroll.driver, Some(Pane::Source));
    app.rich.hold_parser_for_test(false);
    std::thread::sleep(std::time::Duration::from_millis(200));
    for _ in 0..12 {
        render(&mut app, &ctx, &mut frame, vec![]);
    }
    assert!(app.large.split_scroll.current);
    assert!(app.large.split_scroll.anchor.is_some());
    assert_eq!(app.large.split_scroll.driver, Some(Pane::Source));
}
#[test]
fn text_inside_long_code_and_single_wrapped_paragraph_is_continuously_addressable() {
    for code in [false, true] {
        let ctx = egui::Context::default();
        let marker = "位置0250";
        let body = (0..500)
            .map(|i| {
                format!(
                    "位置{i:04} 中文 long mapped content {}",
                    if code { "\n" } else { " " }
                )
            })
            .collect::<String>();
        let text = if code {
            format!("# Long code\n\n```text\n{body}```\n\nEnd\n")
        } else {
            format!("# Long paragraph\n\n{body}\n\nEnd\n")
        };
        let byte = text.find(marker).unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, &text);
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..8 {
            render(&mut app, &ctx, &mut frame, vec![]);
        }
        for driver in [Pane::Source, Pane::Preview] {
            app.large.split_scroll.driver = Some(driver);
            app.large.split_scroll.anchor = Some(Anchor {
                byte,
                fraction: 0.,
                viewport_y: 100.,
                text: true,
            });
            for _ in 0..12 {
                render(&mut app, &ctx, &mut frame, vec![]);
            }
            let follower = if driver == Pane::Source {
                Pane::Preview
            } else {
                Pane::Source
            };
            let v = &app.large.split_scroll.views[follower.index()];
            let y = v
                .galleys
                .iter()
                .find_map(|g| {
                    let at = g.galley.text().find(marker)?;
                    Some(
                        g.pos.y
                            + g.galley
                                .pos_from_cursor(egui::text::CCursor::new(
                                    g.galley.text()[..at].chars().count(),
                                ))
                                .top()
                            - v.rect?.top(),
                    )
                })
                .expect("interior labelled text must be mounted");
            assert!(
                (y - 100.).abs() <= app.font_size * 1.5,
                "code={code} driver={driver:?} actual {y}"
            );
        }
    }
}
#[test]
fn independent_annotated_markdown_position_maps_both_directions_and_clamps() {
    let ctx = egui::Context::default();
    let mut s = SplitScroll::default();
    let source = "# Heading\n\n**Target** after\n";
    let byte = 13; // T in the independently annotated source, after two stars.
    assert_eq!(&source[byte..byte + 6], "Target");
    s.session(Some((1, 1)), 0, true, 1, true);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500., 400.));
    let _ = ctx.run(Default::default(), |ctx| {
        let galley = ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                "Target after".into(),
                egui::FontId::monospace(15.),
                egui::Color32::BLACK,
            )
        });
        let mut cache = crate::reading_geometry::GeometryCache::default();
        let mapped = cache.linear(galley, egui::pos2(0., 120.), byte);
        s.views[1] = View {
            rect: Some(rect),
            offset: 80.,
            max: 800.,
            galleys: vec![mapped],
            revision: Some(0),
            ..Default::default()
        };
        s.driver = Some(Pane::Source);
        s.anchor = Some(Anchor {
            byte,
            fraction: 0.,
            viewport_y: 20.,
            text: true,
        });
        assert_eq!(s.target(Pane::Preview, source, ctx, |_| None), Some(180.));
        s.views[1].max = 100.;
        assert_eq!(s.target(Pane::Preview, source, ctx, |_| None), Some(100.));
        s.intent = Some(Pane::Preview);
        assert!(s.target(Pane::Preview, source, ctx, |_| None).is_none());
    });
}
#[test]
fn programmatic_offsets_and_ordinary_clicks_cannot_acquire_or_replace_driver() {
    let ctx = egui::Context::default();
    let mut s = SplitScroll::default();
    s.session(Some((1, 1)), 0, true, 1, false);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500., 500.));
    s.views[0].rect = Some(rect);
    s.driver = Some(Pane::Preview);
    s.anchor = Some(Anchor {
        byte: 77,
        fraction: 0.,
        viewport_y: 15.,
        text: true,
    });
    let _ = ctx.run(
        egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(egui::pos2(100., 100.)),
                egui::Event::PointerButton {
                    pos: egui::pos2(100., 100.),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            s.input(ctx);
            assert!(s.intent.is_none());
            egui::CentralPanel::default().show(ctx, |ui| {
                s.observe(
                    Pane::Source,
                    ui,
                    egui::Id::new("test-scroll"),
                    rect,
                    400.,
                    400.,
                    1000.,
                    vec![],
                    "",
                    0,
                    true,
                    true,
                    Anchor {
                        byte: 0,
                        fraction: 0.,
                        viewport_y: 0.,
                        text: false,
                    },
                );
            });
        },
    );
    assert_eq!(s.driver, Some(Pane::Preview));
    assert_eq!(s.anchor.unwrap().byte, 77);
    assert_eq!(s.sequence, 0);
}

fn render_native_fps(
    app: &mut crate::Editor,
    ctx: &egui::Context,
    frame: &mut u64,
    fps: u64,
    events: Vec<egui::Event>,
) {
    *frame += 1;
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1240., 819.2),
            )),
            time: Some(*frame as f64 / fps as f64),
            predicted_dt: 1. / fps as f32,
            events,
            ..Default::default()
        },
        |ctx| app.update(ctx, &mut eframe::Frame::_new_kittest()),
    );
}
#[test]
fn line_native_fixture_fps_sweep_keeps_original_marker() {
    line_native_fixture_marker_cycles(&[30, 60, 75, 120], 100);
}

#[test]
fn line_native_fixture_three_hundred_cycles_keep_original_marker() {
    line_native_fixture_marker_cycles(&[30, 120], 300);
}

fn line_native_fixture_marker_cycles(rates: &[u64], cycles: usize) {
    for &fps in rates {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.25);
        let marker = "ANCHOR_0250";
        let text = format!(
            "# 独立代码位置标注\n\n```text\n{}```\n\nEND\n",
            (0..400)
                .map(|i| format!("let ANCHOR_{i:04} = {i};\n"))
                .collect::<String>()
        );
        let byte = text.find(marker).unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, &text);
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..8 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
        for driver in [Pane::Source, Pane::Preview] {
            app.large.split_scroll.driver = Some(driver);
            app.large.split_scroll.anchor = Some(Anchor {
                byte,
                fraction: 0.,
                viewport_y: 4.,
                text: true,
            });
            for _ in 0..8 {
                render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            }
        }
        let mark = |s: &SplitScroll, pane: Pane| {
            let view = &s.views[pane.index()];
            view.galleys
                .iter()
                .find_map(|g| {
                    let at = g.galley.text().find(marker)?;
                    let rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                        g.galley.text()[..at].chars().count(),
                    ));
                    Some((g.pos.y + rect.top() - view.rect?.top(), rect.height()))
                })
                .unwrap()
        };
        let initial = [
            mark(&app.large.split_scroll, Pane::Source),
            mark(&app.large.split_scroll, Pane::Preview),
        ];
        for cycle in 0..cycles {
            let amount = if cycle < 10 { 0.1 } else { 1. };
            for pane in [Pane::Source, Pane::Preview] {
                for delta in [-amount, amount] {
                    let pos = app.large.split_scroll.views[pane.index()]
                        .rect
                        .unwrap()
                        .center();
                    render_native_fps(
                        &mut app,
                        &ctx,
                        &mut frame,
                        fps,
                        vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Line,
                                delta: egui::vec2(0., delta),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                    for _ in 0..((fps as f64 * 0.18).round() as usize).saturating_sub(1) {
                        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
                    }
                }
            }
            for _ in 0..(fps as f64 * 0.25).round() as usize {
                render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            }
            let sync = &app.large.split_scroll;
            let actual = [mark(sync, Pane::Source), mark(sync, Pane::Preview)];
            eprintln!(
                "fps={fps} cycle={cycle} original={initial:?} actual={actual:?} anchor={:?} offsets=[{},{}] residual={:?}",
                sync.anchor,
                sync.views[0].offset,
                sync.views[1].offset,
                sync.anchor.map(|a| [
                    sync.precise(Pane::Source, a, &app.doc.text, frame)
                        .map(|v| v - sync.views[0].offset),
                    sync.precise(Pane::Preview, a, &app.doc.text, frame)
                        .map(|v| v - sync.views[1].offset)
                ])
            );
            for pane in [Pane::Source, Pane::Preview] {
                assert!(
                    (actual[pane.index()].0 - initial[pane.index()].0).abs()
                        <= actual[pane.index()].1,
                    "Line wheel drift cycle {cycle} {pane:?}: {actual:?}"
                );
            }
        }
    }
}

#[test]
fn line_native_fixture_long_wait_drains_smoothing_tail() {
    for fps in [30u64] {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.25);
        let marker = "ANCHOR_0250";
        let text = format!(
            "# 独立代码位置标注\n\n```text\n{}```\n\nEND\n",
            (0..400)
                .map(|i| format!("let ANCHOR_{i:04} = {i};\n"))
                .collect::<String>()
        );
        let byte = text.find(marker).unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, &text);
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..8 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
        for driver in [Pane::Source, Pane::Preview] {
            app.large.split_scroll.driver = Some(driver);
            app.large.split_scroll.anchor = Some(Anchor {
                byte,
                fraction: 0.,
                viewport_y: 4.,
                text: true,
            });
            for _ in 0..8 {
                render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            }
        }
        let mark = |s: &SplitScroll, pane: Pane| {
            let view = &s.views[pane.index()];
            view.galleys
                .iter()
                .find_map(|g| {
                    let at = g.galley.text().find(marker)?;
                    let rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                        g.galley.text()[..at].chars().count(),
                    ));
                    Some((g.pos.y + rect.top() - view.rect?.top(), rect.height()))
                })
                .unwrap()
        };
        let initial = [
            mark(&app.large.split_scroll, Pane::Source),
            mark(&app.large.split_scroll, Pane::Preview),
        ];
        for cycle in 0..100 {
            let amount = if cycle < 10 { 0.1 } else { 1. };
            for pane in [Pane::Source, Pane::Preview] {
                for delta in [-amount, amount] {
                    let pos = app.large.split_scroll.views[pane.index()]
                        .rect
                        .unwrap()
                        .center();
                    render_native_fps(
                        &mut app,
                        &ctx,
                        &mut frame,
                        fps,
                        vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Line,
                                delta: egui::vec2(0., delta),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                    for _ in 0..((fps as f64 * 0.5).round() as usize).saturating_sub(1) {
                        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
                    }
                }
            }
            for _ in 0..(fps as f64 * 0.25).round() as usize {
                render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            }
            let sync = &app.large.split_scroll;
            let actual = [mark(sync, Pane::Source), mark(sync, Pane::Preview)];
            eprintln!(
                "fps={fps} cycle={cycle} original={initial:?} actual={actual:?} anchor={:?} offsets=[{},{}] residual={:?}",
                sync.anchor,
                sync.views[0].offset,
                sync.views[1].offset,
                sync.anchor.map(|a| [
                    sync.precise(Pane::Source, a, &app.doc.text, frame)
                        .map(|v| v - sync.views[0].offset),
                    sync.precise(Pane::Preview, a, &app.doc.text, frame)
                        .map(|v| v - sync.views[1].offset)
                ])
            );
            for pane in [Pane::Source, Pane::Preview] {
                assert!(
                    (actual[pane.index()].0 - initial[pane.index()].0).abs()
                        <= actual[pane.index()].1,
                    "Line wheel drift cycle {cycle} {pane:?}: {actual:?}"
                );
            }
        }
    }
}

#[test]
fn vertical_wheel_uses_fresh_raw_without_cross_pane_tail_and_preserves_other_input() {
    let ctx = egui::Context::default();
    let mut sync = SplitScroll::default();
    sync.session(Some((1, 1)), 0, true, 1, false);
    sync.views[0].rect = Some(egui::Rect::from_min_size(
        egui::pos2(0., 0.),
        egui::vec2(300., 400.),
    ));
    sync.views[1].rect = Some(egui::Rect::from_min_size(
        egui::pos2(310., 0.),
        egui::vec2(300., 400.),
    ));
    sync.driver = Some(Pane::Source);
    for (frame, pane, delta) in [
        (1, Pane::Source, Some(-1.)),
        (2, Pane::Preview, None),
        (3, Pane::Preview, Some(1.)),
    ] {
        let mut events = vec![egui::Event::PointerMoved(
            sync.views[pane.index()].rect.unwrap().center(),
        )];
        if let Some(delta) = delta {
            events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.25, delta),
                modifiers: egui::Modifiers::NONE,
            });
        }
        let _ = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 / 60.),
                predicted_dt: 1. / 60.,
                events,
                ..Default::default()
            },
            |ctx| {
                let (raw, before_x, events) = ctx.input(|i| {
                    (
                        i.raw_scroll_delta.y,
                        i.smooth_scroll_delta.x,
                        i.events.clone(),
                    )
                });
                sync.input(ctx);
                ctx.input(|i| {
                    assert_eq!(i.smooth_scroll_delta.y, raw);
                    assert_eq!(i.smooth_scroll_delta.x, before_x);
                    assert_eq!(i.events, events);
                });
                if delta.is_none() {
                    assert!(
                        sync.intent.is_none(),
                        "hover alone must not acquire the old smooth tail"
                    );
                } else {
                    assert_eq!(sync.intent, Some(pane));
                }
                assert_eq!(
                    sync.driver,
                    Some(Pane::Source),
                    "only actual body consumption may acquire driver"
                );
            },
        );
    }
    for (enabled, current, ctrl) in [(false, true, false), (true, true, true)] {
        sync.session(enabled.then_some((1, 1)), 0, current, 1, false);
        sync.views[0].rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(300., 400.),
        ));
        let modifiers = egui::Modifiers {
            ctrl,
            ..Default::default()
        };
        let _ = ctx.run(
            egui::RawInput {
                modifiers,
                events: vec![
                    egui::Event::PointerMoved(egui::pos2(100., 100.)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::vec2(0., -1.),
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                let before =
                    ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.events.clone()));
                sync.input(ctx);
                assert_eq!(
                    ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.events.clone())),
                    before,
                    "non-target, stale and Ctrl zoom must retain framework input"
                );
            },
        );
    }
}

#[test]
fn stale_split_hover_cannot_transfer_old_wheel_tail() {
    let ctx = egui::Context::default();
    let mut sync = SplitScroll::default();
    sync.session(Some((1, 1)), 1, false, 1, false);
    sync.views[0].rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(300., 400.),
    ));
    sync.views[1].rect = Some(egui::Rect::from_min_size(
        egui::pos2(310., 0.),
        egui::vec2(300., 400.),
    ));
    for (frame, pane, fresh) in [(1, Pane::Source, true), (2, Pane::Preview, false)] {
        let mut events = vec![egui::Event::PointerMoved(
            sync.views[pane.index()].rect.unwrap().center(),
        )];
        if fresh {
            events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0., -1.),
                modifiers: egui::Modifiers::NONE,
            });
        }
        let _ = ctx.run(
            egui::RawInput {
                time: Some(frame as f64 / 60.),
                predicted_dt: 1. / 60.,
                events,
                ..Default::default()
            },
            |ctx| {
                sync.input(ctx);
                if fresh {
                    assert_eq!(sync.intent, Some(Pane::Source));
                } else {
                    assert!(
                        sync.intent.is_none(),
                        "stale hover must not become a latest user intent from old smoothing"
                    );
                }
                assert!(
                    sync.anchor.is_none(),
                    "old source coordinates remain forbidden"
                );
            },
        );
    }
}

#[test]
fn nested_vertical_scroll_consumes_raw_without_acquiring_body_driver() {
    let ctx = egui::Context::default();
    let mut sync = SplitScroll::default();
    sync.session(Some((1, 1)), 0, true, 1, false);
    sync.views[0].rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(600., 400.),
    ));
    sync.driver = Some(Pane::Preview);
    let mut last_inner = 0.;
    for frame in 0..2 {
        let mut events = vec![egui::Event::PointerMoved(egui::pos2(60., 60.))];
        if frame == 1 {
            events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0., -1.),
                modifiers: egui::Modifiers::NONE,
            });
        }
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600., 400.),
                )),
                time: Some(frame as f64 / 60.),
                events,
                ..Default::default()
            },
            |ctx| {
                sync.input(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut inner = 0.;
                    let mut drawn = 0.;
                    let outer = egui::ScrollArea::vertical()
                        .id_salt("nested-outer")
                        .max_height(300.)
                        .show_viewport(ui, |ui, viewport| {
                            drawn = viewport.min.y;
                            let result = egui::ScrollArea::vertical()
                                .id_salt("nested-inner")
                                .max_height(120.)
                                .show(ui, |ui| {
                                    ui.allocate_space(egui::vec2(250., 1200.));
                                });
                            inner = result.state.offset.y;
                            ui.allocate_space(egui::vec2(250., 1600.));
                        });
                    assert_eq!(
                        outer.state.offset.y, 0.,
                        "nested input must not reach the body"
                    );
                    if frame == 1 {
                        assert!(
                            inner > last_inner,
                            "actual nested ScrollArea must consume the input"
                        );
                    }
                    last_inner = inner;
                    sync.observe(
                        Pane::Source,
                        ui,
                        egui::Id::new("nested-outer"),
                        outer.inner_rect,
                        drawn,
                        outer.state.offset.y,
                        1000.,
                        vec![],
                        "",
                        0,
                        true,
                        false,
                        Anchor {
                            byte: 0,
                            fraction: 0.,
                            viewport_y: 0.,
                            text: false,
                        },
                    );
                    assert_eq!(sync.driver, Some(Pane::Preview));
                    assert_eq!(sync.sequence, 0);
                });
            },
        );
    }
}

#[test]
fn short_split_source_cannot_scroll_its_actual_text_out_of_view() {
    for font in [15., 23.] {
        for text in ["", "# Short\n\n中文短稿。\n"] {
            let ctx = egui::Context::default();
            let mut app = crate::preview_layout_tests::editor(&ctx, text);
            app.font_size = font;
            let mut frame = 0;
            app.reading.bind(&mut app.doc, "main", 0);
            for _ in 0..8 {
                render(&mut app, &ctx, &mut frame, vec![]);
            }
            wheel(&mut app, &ctx, &mut frame, Pane::Source, -120.);
            for _ in 0..12 {
                render(&mut app, &ctx, &mut frame, vec![]);
            }
            let view = &app.large.split_scroll.views[0];
            assert!(
                view.max <= 0.5,
                "short source manufactured {}px scroll range at font{font}",
                view.max
            );
            assert!(view.offset.abs() <= 0.01);
            if !text.is_empty() {
                let rect = view.rect.unwrap();
                let g = view.galleys.first().expect("source Galley");
                assert!(g.galley.text().contains("Short"));
                let y = g.pos.y + g.galley.pos_from_cursor(egui::text::CCursor::new(0)).top();
                assert!(
                    y >= rect.top() - 1. && y < rect.bottom(),
                    "actual short literal is clipped"
                );
            }
            let pos = app.large.split_scroll.views[0].rect.unwrap().center();
            render(
                &mut app,
                &ctx,
                &mut frame,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            render(
                &mut app,
                &ctx,
                &mut frame,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert_eq!(
                ctx.memory(|m| m.focused()),
                Some(egui::Id::new(("source-editor", app.document_generation))),
                "blank source area remains clickable"
            );
        }
    }
}

#[test]
fn table_native_takeovers_keep_the_same_independent_row() {
    for fps in [60u64] {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.25);
        let marker = "ANCHOR_0250";
        let text = format!(
            "# Single long table\n\n| ID | 中文说明 |\n| --- | --- |\n{}",
            (0..1500)
                .map(|i| format!("| ANCHOR_{i:04} | 中文表格连续映射内容第{i:04}行。 |\n"))
                .collect::<String>()
        );
        let byte = text.find(marker).unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, &text);
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..400 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            if app.large.split_scroll.current && !app.large.split_scroll.views[1].galleys.is_empty()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        // Mount the independently labelled source row using real native input
        // before the one-time paired anchor setup; table rows are virtualized.
        for _ in 0..80 {
            let v = &app.large.split_scroll.views[0];
            let visible = v.rect.is_some_and(|rect| {
                v.galleys.iter().any(|g| {
                    g.galley.text().find(marker).is_some_and(|at| {
                        let y = g.pos.y
                            + g.galley
                                .pos_from_cursor(egui::text::CCursor::new(
                                    g.galley.text()[..at].chars().count(),
                                ))
                                .top();
                        y >= rect.top() && y < rect.bottom()
                    })
                })
            });
            if visible {
                break;
            }
            let pos = v.rect.expect("source viewport mounted").center();
            render_native_fps(
                &mut app,
                &ctx,
                &mut frame,
                fps,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::vec2(0., -2.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            for _ in 0..12 {
                render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            }
        }
        for driver in [Pane::Source, Pane::Preview] {
            app.large.split_scroll.navigating();
            app.large.split_scroll.driver = Some(driver);
            app.large.split_scroll.anchor = Some(Anchor {
                byte,
                fraction: 0.,
                viewport_y: 4.,
                text: true,
            });
            for _ in 0..8 {
                render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            }
        }
        let mark = |s: &SplitScroll, pane: Pane| {
            let view = &s.views[pane.index()];
            view.galleys
                .iter()
                .find_map(|g| {
                    let at = g.galley.text().find(marker)?;
                    let rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                        g.galley.text()[..at].chars().count(),
                    ));
                    Some((g.pos.y + rect.top() - view.rect?.top(), rect.height()))
                })
                .unwrap()
        };
        let initial = [
            mark(&app.large.split_scroll, Pane::Source),
            mark(&app.large.split_scroll, Pane::Preview),
        ];
        for cycle in 0..20 {
            let amount = if cycle < 10 { 0.1 } else { 1. };
            for pane in [Pane::Source, Pane::Preview] {
                for delta in [-amount, amount] {
                    let pos = app.large.split_scroll.views[pane.index()]
                        .rect
                        .unwrap()
                        .center();
                    render_native_fps(
                        &mut app,
                        &ctx,
                        &mut frame,
                        fps,
                        vec![
                            egui::Event::PointerMoved(pos),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Line,
                                delta: egui::vec2(0., delta),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                    for _ in 0..((fps as f64 * 0.18).round() as usize).saturating_sub(1) {
                        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
                    }
                }
            }
            for _ in 0..(fps as f64 * 0.25).round() as usize {
                render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
            }
            let sync = &app.large.split_scroll;
            let actual = [mark(sync, Pane::Source), mark(sync, Pane::Preview)];
            eprintln!(
                "fps={fps} cycle={cycle} original={initial:?} actual={actual:?} anchor={:?} offsets=[{},{}] residual={:?}",
                sync.anchor,
                sync.views[0].offset,
                sync.views[1].offset,
                sync.anchor.map(|a| [
                    sync.precise(Pane::Source, a, &app.doc.text, frame)
                        .map(|v| v - sync.views[0].offset),
                    sync.precise(Pane::Preview, a, &app.doc.text, frame)
                        .map(|v| v - sync.views[1].offset)
                ])
            );
            for pane in [Pane::Source, Pane::Preview] {
                assert!(
                    (actual[pane.index()].0 - initial[pane.index()].0).abs()
                        <= actual[pane.index()].1,
                    "Line wheel drift cycle {cycle} {pane:?}: {actual:?}"
                );
            }
        }
    }
}

// Independent oracle: labels come only from the actually painted galleys.
// No controller coordinate or mapping function is used to derive expectation.
fn assert_table_fraction(s: &SplitScroll, driver: Pane) {
    let collect = |pane: Pane| {
        let view = &s.views[pane.index()];
        let viewport = view.rect.expect("painted viewport");
        let mut rows = std::collections::BTreeMap::new();
        for g in &view.galleys {
            for (at, _) in g.galley.text().match_indices("ANCHOR_") {
                let label = &g.galley.text()[at..at + 11];
                let glyph = g.galley.pos_from_cursor(egui::text::CCursor::new(
                    g.galley.text()[..at].chars().count(),
                ));
                rows.insert(
                    label.to_owned(),
                    (g.pos.y + glyph.top() - viewport.top(), glyph.height()),
                );
            }
        }
        rows
    };
    let source = collect(driver);
    let follower = if driver == Pane::Source {
        Pane::Preview
    } else {
        Pane::Source
    };
    let other = collect(follower);
    let rows: Vec<_> = source.iter().collect();
    let pair = rows
        .windows(2)
        .find(|p| p[0].1.0 <= 0. && p[1].1.0 > 0.)
        .expect("two actual adjacent labelled rows must surround the driver viewport top");
    let (start, &(a, _)) = pair[0];
    let (end, &(b, _)) = pair[1];
    let &(c, line) = other
        .get(start)
        .expect("same preceding actual row mounted on follower");
    let &(d, _) = other
        .get(end)
        .expect("same following actual row mounted on follower");
    let fraction = -a / (b - a);
    let error = c + (d - c) * fraction;
    eprintln!(
        "ROW_ORACLE driver={driver:?} rows={start},{end} driver_bounds={a},{b} follower_bounds={c},{d} fraction={fraction} point_error={error} actual_line={line}"
    );
    assert!(
        error.abs() <= line,
        "independent semantic row point exceeds one actual line"
    );
}

fn table_single_step_trajectory(reduce: bool, amount: f32, driver: Pane) {
    let fps = 60u64;
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(1.25);
    let marker = "ANCHOR_0250";
    let text = format!(
        "# Single long table\n\n| ID | 中文说明 |\n| --- | --- |\n{}",
        (0..1500)
            .map(|i| format!("| ANCHOR_{i:04} | 中文表格连续映射内容第{i:04}行。 |\n"))
            .collect::<String>()
    );
    let byte = text.find(marker).unwrap();
    let mut app = crate::preview_layout_tests::editor(&ctx, &text);
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..400 {
        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        if app.large.split_scroll.current && !app.large.split_scroll.views[1].galleys.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // Mount the independently labelled source row using real native input
    // before the one-time paired anchor setup; table rows are virtualized.
    for _ in 0..80 {
        let v = &app.large.split_scroll.views[0];
        let visible = v.rect.is_some_and(|rect| {
            v.galleys.iter().any(|g| {
                g.galley.text().find(marker).is_some_and(|at| {
                    let y = g.pos.y
                        + g.galley
                            .pos_from_cursor(egui::text::CCursor::new(
                                g.galley.text()[..at].chars().count(),
                            ))
                            .top();
                    y >= rect.top() && y < rect.bottom()
                })
            })
        });
        if visible {
            break;
        }
        let pos = v.rect.expect("source viewport mounted").center();
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            fps,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0., -2.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..12 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
    }
    for driver in [Pane::Source, Pane::Preview] {
        app.large.split_scroll.navigating();
        app.large.split_scroll.driver = Some(driver);
        app.large.split_scroll.anchor = Some(Anchor {
            byte,
            fraction: 0.,
            viewport_y: 4.,
            text: true,
        });
        for _ in 0..8 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
    }
    let mark = |s: &SplitScroll, pane: Pane| {
        let view = &s.views[pane.index()];
        view.galleys
            .iter()
            .find_map(|g| {
                let at = g.galley.text().find(marker)?;
                let rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                    g.galley.text()[..at].chars().count(),
                ));
                Some((g.pos.y + rect.top() - view.rect?.top(), rect.height()))
            })
            .unwrap()
    };
    let _initial = [
        mark(&app.large.split_scroll, Pane::Source),
        mark(&app.large.split_scroll, Pane::Preview),
    ];

    app.preferences.reduce_motion = reduce;
    let glyph = |s: &SplitScroll, pane: Pane, label: &str| -> Option<(f32, f32)> {
        let v = &s.views[pane.index()];
        v.galleys.iter().find_map(|g| {
            let at = g.galley.text().find(label)?;
            let r = g.galley.pos_from_cursor(egui::text::CCursor::new(
                g.galley.text()[..at].chars().count(),
            ));
            Some((g.pos.y + r.top() - v.rect?.top(), r.height()))
        })
    };
    let mut track = vec![[
        glyph(&app.large.split_scroll, Pane::Source, "ANCHOR_0258")
            .unwrap()
            .0,
        glyph(&app.large.split_scroll, Pane::Preview, "ANCHOR_0258")
            .unwrap()
            .0,
    ]];
    let pos = app.large.split_scroll.views[driver.index()]
        .rect
        .unwrap()
        .center();
    render_native_fps(
        &mut app,
        &ctx,
        &mut frame,
        fps,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0., -amount / 40.),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..24 {
        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        track.push([
            glyph(&app.large.split_scroll, Pane::Source, "ANCHOR_0258")
                .unwrap()
                .0,
            glyph(&app.large.split_scroll, Pane::Preview, "ANCHOR_0258")
                .unwrap()
                .0,
        ]);
    }
    eprintln!("TRAJECTORY reduce={reduce} amount={amount} driver={driver:?} glyph258={track:?}");
    for pair in track.windows(2) {
        for side in 0..2 {
            assert!(
                pair[1][side] <= pair[0][side] + 1.,
                "follower bounced opposite to native direction: {pair:?}"
            );
        }
    }
    let v = &app.large.split_scroll.views[driver.index()];
    let mut top: Option<(String, f32)> = None;
    for g in &v.galleys {
        for (at, _) in g.galley.text().match_indices("ANCHOR_") {
            let label = g.galley.text()[at..at + 11].to_owned();
            let r = g.galley.pos_from_cursor(egui::text::CCursor::new(
                g.galley.text()[..at].chars().count(),
            ));
            let y = g.pos.y + r.top() - v.rect.unwrap().top();
            if y >= -0.1
                && y < v.rect.unwrap().height()
                && top.as_ref().is_none_or(|(_, prior)| y < *prior)
            {
                top = Some((label, y));
            }
        }
    }
    let (label, y) = top.expect("driver visible table label");
    let follower = if driver == Pane::Source {
        Pane::Preview
    } else {
        Pane::Source
    };
    let (other, line) = glyph(&app.large.split_scroll, follower, &label)
        .expect("driver's top content must be mounted on follower");
    eprintln!(
        "STOP reduce={reduce} amount={amount} driver={driver:?} label={label} driver_y={y} follower_y={other} line={line}"
    );
    assert_table_fraction(&app.large.split_scroll, driver);
}
#[test]
fn table_step_40_animated() {
    table_single_step_trajectory(false, 40., Pane::Source)
}
#[test]
fn table_step_120_animated() {
    table_single_step_trajectory(false, 120., Pane::Source)
}
#[test]
fn table_step_40_reduced() {
    table_single_step_trajectory(true, 40., Pane::Preview)
}
#[test]
fn table_step_120_reduced() {
    table_single_step_trajectory(true, 120., Pane::Preview)
}

#[test]
fn table_step_40_animated_preview() {
    table_single_step_trajectory(false, 40., Pane::Preview)
}
#[test]
fn table_step_120_animated_preview() {
    table_single_step_trajectory(false, 120., Pane::Preview)
}
#[test]
fn table_step_40_reduced_source() {
    table_single_step_trajectory(true, 40., Pane::Source)
}
#[test]
fn table_step_120_reduced_source() {
    table_single_step_trajectory(true, 120., Pane::Source)
}

fn table_reverse_step_trajectory(reduce: bool, amount: f32, driver: Pane) {
    let fps = 60u64;
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(1.25);
    let marker = "ANCHOR_0250";
    let text = format!(
        "# Single long table\n\n| ID | 中文说明 |\n| --- | --- |\n{}",
        (0..1500)
            .map(|i| format!("| ANCHOR_{i:04} | 中文表格连续映射内容第{i:04}行。 |\n"))
            .collect::<String>()
    );
    let byte = text.find(marker).unwrap();
    let mut app = crate::preview_layout_tests::editor(&ctx, &text);
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..400 {
        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        if app.large.split_scroll.current && !app.large.split_scroll.views[1].galleys.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // Mount the independently labelled source row using real native input
    // before the one-time paired anchor setup; table rows are virtualized.
    for _ in 0..80 {
        let v = &app.large.split_scroll.views[0];
        let visible = v.rect.is_some_and(|rect| {
            v.galleys.iter().any(|g| {
                g.galley.text().find(marker).is_some_and(|at| {
                    let y = g.pos.y
                        + g.galley
                            .pos_from_cursor(egui::text::CCursor::new(
                                g.galley.text()[..at].chars().count(),
                            ))
                            .top();
                    y >= rect.top() && y < rect.bottom()
                })
            })
        });
        if visible {
            break;
        }
        let pos = v.rect.expect("source viewport mounted").center();
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            fps,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0., -2.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..12 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
    }
    for driver in [Pane::Source, Pane::Preview] {
        app.large.split_scroll.navigating();
        app.large.split_scroll.driver = Some(driver);
        app.large.split_scroll.anchor = Some(Anchor {
            byte,
            fraction: 0.,
            viewport_y: -4.,
            text: true,
        });
        for _ in 0..8 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
    }
    let mark = |s: &SplitScroll, pane: Pane| {
        let view = &s.views[pane.index()];
        view.galleys
            .iter()
            .find_map(|g| {
                let at = g.galley.text().find(marker)?;
                let rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                    g.galley.text()[..at].chars().count(),
                ));
                Some((g.pos.y + rect.top() - view.rect?.top(), rect.height()))
            })
            .unwrap()
    };
    let _initial = [
        mark(&app.large.split_scroll, Pane::Source),
        mark(&app.large.split_scroll, Pane::Preview),
    ];

    app.preferences.reduce_motion = reduce;
    let prep = app.large.split_scroll.views[driver.index()]
        .rect
        .unwrap()
        .center();
    render_native_fps(
        &mut app,
        &ctx,
        &mut frame,
        fps,
        vec![
            egui::Event::PointerMoved(prep),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0., -amount / 40.),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..24 {
        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
    }
    let glyph = |s: &SplitScroll, pane: Pane, label: &str| -> Option<(f32, f32)> {
        let v = &s.views[pane.index()];
        v.galleys.iter().find_map(|g| {
            let at = g.galley.text().find(label)?;
            let r = g.galley.pos_from_cursor(egui::text::CCursor::new(
                g.galley.text()[..at].chars().count(),
            ));
            Some((g.pos.y + r.top() - v.rect?.top(), r.height()))
        })
    };
    let mut track = vec![[
        glyph(&app.large.split_scroll, Pane::Source, "ANCHOR_0258")
            .unwrap()
            .0,
        glyph(&app.large.split_scroll, Pane::Preview, "ANCHOR_0258")
            .unwrap()
            .0,
    ]];
    let pos = app.large.split_scroll.views[driver.index()]
        .rect
        .unwrap()
        .center();
    render_native_fps(
        &mut app,
        &ctx,
        &mut frame,
        fps,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0., amount / 40.),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..24 {
        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        track.push([
            glyph(&app.large.split_scroll, Pane::Source, "ANCHOR_0258")
                .unwrap()
                .0,
            glyph(&app.large.split_scroll, Pane::Preview, "ANCHOR_0258")
                .unwrap()
                .0,
        ]);
    }
    eprintln!("TRAJECTORY reduce={reduce} amount={amount} driver={driver:?} glyph258={track:?}");
    for pair in track.windows(2) {
        for side in 0..2 {
            assert!(
                pair[1][side] >= pair[0][side] - 1.,
                "follower bounced opposite to native direction: {pair:?}"
            );
        }
    }
    let v = &app.large.split_scroll.views[driver.index()];
    let mut top: Option<(String, f32)> = None;
    for g in &v.galleys {
        for (at, _) in g.galley.text().match_indices("ANCHOR_") {
            let label = g.galley.text()[at..at + 11].to_owned();
            let r = g.galley.pos_from_cursor(egui::text::CCursor::new(
                g.galley.text()[..at].chars().count(),
            ));
            let y = g.pos.y + r.top() - v.rect.unwrap().top();
            if y >= -0.1
                && y < v.rect.unwrap().height()
                && top.as_ref().is_none_or(|(_, prior)| y < *prior)
            {
                top = Some((label, y));
            }
        }
    }
    let (label, y) = top.expect("driver visible table label");
    let follower = if driver == Pane::Source {
        Pane::Preview
    } else {
        Pane::Source
    };
    let (other, line) = glyph(&app.large.split_scroll, follower, &label)
        .expect("driver's top content must be mounted on follower");
    eprintln!(
        "STOP reduce={reduce} amount={amount} driver={driver:?} label={label} driver_y={y} follower_y={other} line={line}"
    );
    assert_table_fraction(&app.large.split_scroll, driver);
}

#[test]
fn table_reverse_40_animated() {
    table_reverse_step_trajectory(false, 40., Pane::Source)
}
#[test]
fn table_reverse_120_animated() {
    table_reverse_step_trajectory(false, 120., Pane::Preview)
}
#[test]
fn table_reverse_40_reduced() {
    table_reverse_step_trajectory(true, 40., Pane::Preview)
}
#[test]
fn table_reverse_120_reduced() {
    table_reverse_step_trajectory(true, 120., Pane::Source)
}

fn table_continuous_trajectory(reduce: bool, driver: Pane, rate: u64) {
    let fps = rate;
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(1.25);
    let marker = "ANCHOR_0250";
    let text = format!(
        "# Single long table\n\n| ID | 中文说明 |\n| --- | --- |\n{}",
        (0..1500)
            .map(|i| format!("| ANCHOR_{i:04} | 中文表格连续映射内容第{i:04}行。 |\n"))
            .collect::<String>()
    );
    let byte = text.find(marker).unwrap();
    let mut app = crate::preview_layout_tests::editor(&ctx, &text);
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..400 {
        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        if app.large.split_scroll.current && !app.large.split_scroll.views[1].galleys.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // Mount the independently labelled source row using real native input
    // before the one-time paired anchor setup; table rows are virtualized.
    for _ in 0..80 {
        let v = &app.large.split_scroll.views[0];
        let visible = v.rect.is_some_and(|rect| {
            v.galleys.iter().any(|g| {
                g.galley.text().find(marker).is_some_and(|at| {
                    let y = g.pos.y
                        + g.galley
                            .pos_from_cursor(egui::text::CCursor::new(
                                g.galley.text()[..at].chars().count(),
                            ))
                            .top();
                    y >= rect.top() && y < rect.bottom()
                })
            })
        });
        if visible {
            break;
        }
        let pos = v.rect.expect("source viewport mounted").center();
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            fps,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0., -2.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..12 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
    }
    for driver in [Pane::Source, Pane::Preview] {
        app.large.split_scroll.navigating();
        app.large.split_scroll.driver = Some(driver);
        app.large.split_scroll.anchor = Some(Anchor {
            byte,
            fraction: 0.,
            viewport_y: -4.,
            text: true,
        });
        for _ in 0..8 {
            render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
        }
        eprintln!(
            "TABLE_BOOT driver={driver:?} anchor={:?} offsets={:?} first={:?}",
            app.large.split_scroll.anchor,
            app.large.split_scroll.views.each_ref().map(|v| v.offset),
            app.large
                .split_scroll
                .views
                .each_ref()
                .map(|v| v.galleys.first().map(|g| g
                    .galley
                    .text()
                    .chars()
                    .take(40)
                    .collect::<String>()))
        );
    }
    let mark = |s: &SplitScroll, pane: Pane| {
        let view = &s.views[pane.index()];
        view.galleys
            .iter()
            .find_map(|g| {
                let at = g.galley.text().find(marker)?;
                let rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                    g.galley.text()[..at].chars().count(),
                ));
                Some((g.pos.y + rect.top() - view.rect?.top(), rect.height()))
            })
            .unwrap()
    };
    let _initial = [
        mark(&app.large.split_scroll, Pane::Source),
        mark(&app.large.split_scroll, Pane::Preview),
    ];

    app.preferences.reduce_motion = reduce;
    let glyph = |s: &SplitScroll, pane: Pane, label: &str| -> Option<(f32, f32)> {
        let v = &s.views[pane.index()];
        v.galleys.iter().find_map(|g| {
            let at = g.galley.text().find(label)?;
            let r = g.galley.pos_from_cursor(egui::text::CCursor::new(
                g.galley.text()[..at].chars().count(),
            ));
            Some((g.pos.y + r.top() - v.rect?.top(), r.height()))
        })
    };
    let mut track = vec![[
        glyph(&app.large.split_scroll, Pane::Source, "ANCHOR_0258")
            .unwrap()
            .0,
        glyph(&app.large.split_scroll, Pane::Preview, "ANCHOR_0258")
            .unwrap()
            .0,
    ]];
    let pos = app.large.split_scroll.views[driver.index()]
        .rect
        .unwrap()
        .center();
    for delta in [-4., 4.] {
        for _ in 0..10 {
            render_native_fps(
                &mut app,
                &ctx,
                &mut frame,
                fps,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., delta),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            track.push([
                glyph(&app.large.split_scroll, Pane::Source, "ANCHOR_0258")
                    .unwrap()
                    .0,
                glyph(&app.large.split_scroll, Pane::Preview, "ANCHOR_0258")
                    .unwrap()
                    .0,
            ]);
        }
    }
    for _ in 0..24 {
        render_native_fps(&mut app, &ctx, &mut frame, fps, vec![]);
    }
    let amount = 40.;
    eprintln!("TRAJECTORY reduce={reduce} amount={amount} driver={driver:?} glyph258={track:?}");
    let mut last_direction = 0;
    for pair in track.windows(2) {
        let motion = pair[1][driver.index()] - pair[0][driver.index()];
        if motion.abs() > 1. {
            last_direction = if motion > 0. { 1 } else { -1 };
        }
        let follower = 1 - driver.index();
        let following = pair[1][follower] - pair[0][follower];
        assert!(
            last_direction == 0 || following * last_direction as f32 >= -1.,
            "continuous glyph reversal while driver direction is stable: {pair:?}"
        );
    }
    let v = &app.large.split_scroll.views[driver.index()];
    let mut top: Option<(String, f32)> = None;
    for g in &v.galleys {
        for (at, _) in g.galley.text().match_indices("ANCHOR_") {
            let label = g.galley.text()[at..at + 11].to_owned();
            let r = g.galley.pos_from_cursor(egui::text::CCursor::new(
                g.galley.text()[..at].chars().count(),
            ));
            let y = g.pos.y + r.top() - v.rect.unwrap().top();
            if y >= -0.1
                && y < v.rect.unwrap().height()
                && top.as_ref().is_none_or(|(_, prior)| y < *prior)
            {
                top = Some((label, y));
            }
        }
    }
    let (label, y) = top.expect("driver visible table label");
    let follower = if driver == Pane::Source {
        Pane::Preview
    } else {
        Pane::Source
    };
    let (other, line) = glyph(&app.large.split_scroll, follower, &label)
        .expect("driver's top content must be mounted on follower");
    eprintln!(
        "STOP reduce={reduce} amount={amount} driver={driver:?} label={label} driver_y={y} follower_y={other} line={line}"
    );
    assert_table_fraction(&app.large.split_scroll, driver);
}

#[test]
fn table_continuous_source_60_animated() {
    table_continuous_trajectory(false, Pane::Source, 60)
}
#[test]
fn table_continuous_source_120_reduced() {
    table_continuous_trajectory(true, Pane::Source, 120)
}
#[test]
fn table_continuous_preview_60_reduced() {
    table_continuous_trajectory(true, Pane::Preview, 60)
}
#[test]
fn table_continuous_preview_120_animated() {
    table_continuous_trajectory(false, Pane::Preview, 120)
}

#[test]
fn table_tall_terminal_row_uses_current_layout_extent_bidirectionally() {
    for driver in [Pane::Source, Pane::Preview] {
        for reduce in [false, true] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(1.25);
            let text = format!(
                "# Terminal table\n\nID | Value\n--- | ---\nROW_BEFORE | short\nROW_LAST | TERMINAL_VALUE_{}\n\nAfter table\n",
                "中文末行换行内容 ".repeat(300)
            );
            let byte = text.find("ROW_LAST").unwrap();
            let mut app = crate::preview_layout_tests::editor(&ctx, &text);
            let mut frame = 0;
            app.reading.bind(&mut app.doc, "main", 0);
            for _ in 0..400 {
                render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
                if app.large.split_scroll.current
                    && !app.large.split_scroll.views[1].galleys.is_empty()
                {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            for pane in [Pane::Source, Pane::Preview] {
                app.large.split_scroll.navigating();
                app.large.split_scroll.driver = Some(pane);
                app.large.split_scroll.anchor = Some(Anchor {
                    byte,
                    fraction: 0.,
                    viewport_y: -80.,
                    text: true,
                });
                for _ in 0..12 {
                    render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
                }
            }
            app.preferences.reduce_motion = reduce;
            let pos = app.large.split_scroll.views[driver.index()]
                .rect
                .unwrap()
                .center();
            let wheel = |delta| {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., delta),
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            };
            render_native_fps(&mut app, &ctx, &mut frame, 60, wheel(-4.));
            for _ in 0..18 {
                render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
            }
            let bounds = |s: &SplitScroll, pane: Pane| {
                let v = &s.views[pane.index()];
                let rect = v.rect.unwrap();
                if pane == Pane::Source {
                    let g = v
                        .galleys
                        .iter()
                        .find(|g| g.galley.text().contains("ROW_LAST"))
                        .unwrap();
                    let at = g.galley.text().find("ROW_LAST").unwrap();
                    let end = at + g.galley.text()[at..].find('\n').unwrap();
                    let start_rect = g.galley.pos_from_cursor(egui::text::CCursor::new(
                        g.galley.text()[..at].chars().count(),
                    ));
                    let mut cursor =
                        egui::text::CCursor::new(g.galley.text()[..end].chars().count());
                    cursor.prefer_next_row = false;
                    let end_rect = g.galley.pos_from_cursor(cursor);
                    (
                        g.pos.y + start_rect.top() - rect.top(),
                        g.pos.y + end_rect.bottom() - rect.top(),
                        start_rect.height(),
                    )
                } else {
                    let first = v
                        .galleys
                        .iter()
                        .find(|g| g.galley.text().contains("ROW_LAST"))
                        .unwrap();
                    let last = v
                        .galleys
                        .iter()
                        .find(|g| g.galley.text().contains("TERMINAL_VALUE_"))
                        .unwrap();
                    let end = last.galley.rows.last().unwrap();
                    (
                        first.pos.y - rect.top(),
                        last.pos.y + end.rect().bottom() - rect.top(),
                        first.galley.rows[0].height(),
                    )
                }
            };
            let mut previous = [
                bounds(&app.large.split_scroll, Pane::Source).0,
                bounds(&app.large.split_scroll, Pane::Preview).0,
            ];
            for delta in [-4., 4.] {
                for _ in 0..30 {
                    render_native_fps(&mut app, &ctx, &mut frame, 60, wheel(delta));
                    let next = [
                        bounds(&app.large.split_scroll, Pane::Source).0,
                        bounds(&app.large.split_scroll, Pane::Preview).0,
                    ];
                    let motion = next[driver.index()] - previous[driver.index()];
                    if motion.abs() > 1. {
                        assert!(
                            (next[1 - driver.index()] - previous[1 - driver.index()])
                                * motion.signum()
                                >= -1.,
                            "terminal row follower reversed: {previous:?} -> {next:?}"
                        );
                    }
                    previous = next;
                }
            }
            for _ in 0..24 {
                render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
            }
            let (a, b, _) = bounds(&app.large.split_scroll, driver);
            let follower = if driver == Pane::Source {
                Pane::Preview
            } else {
                Pane::Source
            };
            let (c, d, line) = bounds(&app.large.split_scroll, follower);
            assert!(
                a <= 0.
                    && b > app.large.split_scroll.views[driver.index()]
                        .rect
                        .unwrap()
                        .height(),
                "real terminal row surrounds the viewport"
            );
            let fraction = -a / (b - a);
            let error = c + (d - c) * fraction;
            eprintln!(
                "TERMINAL_ORACLE driver={driver:?} reduce={reduce} driver_bounds={a},{b} follower_bounds={c},{d} fraction={fraction} error={error} line={line}"
            );
            assert!(
                error.abs() <= line,
                "terminal row independent layout extent exceeds one actual line"
            );
        }
    }
}

#[test]
fn virtual_source_drag_selection_auto_reveal_stays_local_after_source_wheel_ownership() {
    virtual_source_drag_selection_case(false);
}
#[test]
fn latest_page_command_retains_ownership_then_preview_wheel_takes_over_after_virtual_drag_release()
{
    virtual_source_drag_selection_case(true);
}
fn virtual_source_drag_selection_case(takeover: bool) {
    let ctx = egui::Context::default();
    let text = (0..30000)
        .map(|i| format!("Paragraph {i:06} 中文内容 scrolling.\n\n"))
        .collect::<String>();
    let mut app = crate::preview_layout_tests::editor(&ctx, &text);
    let mut frame = 0;
    app.reading.bind(&mut app.doc, "main", 0);
    for _ in 0..400 {
        render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
        if app.large.split_scroll.current && !app.large.split_scroll.views[1].galleys.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        app.large.active,
        "this regression must exercise virtual source auto-edge selection"
    );
    let viewport = app.large.split_scroll.views[0].rect.unwrap();
    render_native_fps(
        &mut app,
        &ctx,
        &mut frame,
        60,
        vec![
            egui::Event::PointerMoved(viewport.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -320.),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..18 {
        render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
    }
    assert_eq!(app.large.split_scroll.driver, Some(Pane::Source));
    let before = app.large.split_scroll.views.each_ref().map(|v| v.offset);
    let sequence = app.large.split_scroll.sequence;
    let revision = app.doc.revision();
    let dirty = app.doc.dirty();
    let selection = app.large.selection;
    let start = egui::pos2(viewport.left() + 80., viewport.top() + 100.);
    render_native_fps(
        &mut app,
        &ctx,
        &mut frame,
        60,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let end = egui::pos2(start.x, viewport.bottom() + 90.);
    render_native_fps(
        &mut app,
        &ctx,
        &mut frame,
        60,
        vec![egui::Event::PointerMoved(end)],
    );
    for _ in 0..20 {
        render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
    }
    if takeover {
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            60,
            vec![egui::Event::Key {
                key: egui::Key::PageDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        eprintln!(
            "DRAG_TAKEOVER intent={:?} driver={:?} sequence={} offsets={:?} current={}",
            app.large.split_scroll.intent,
            app.large.split_scroll.driver,
            app.large.split_scroll.sequence,
            app.large.split_scroll.views.each_ref().map(|v| v.offset),
            app.large.split_scroll.current
        );
        assert_eq!(
            app.large.split_scroll.driver,
            Some(Pane::Source),
            "older auto-edge drag cannot cancel this pass's consumed page command"
        );
        for _ in 0..8 {
            render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
        }
        assert_eq!(
            app.large.split_scroll.driver,
            Some(Pane::Source),
            "older held drag cannot cancel the next actual drawn capture either"
        );
    }
    render_native_fps(
        &mut app,
        &ctx,
        &mut frame,
        60,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    for _ in 0..8 {
        render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
    }
    let after = app.large.split_scroll.views.each_ref().map(|v| v.offset);
    eprintln!(
        "SELECTION_AUTO_REVEAL before={before:?} after={after:?} sequence={sequence} selection={:?}",
        app.large.selection
    );
    assert!(
        after[0] - before[0] > 100.,
        "real virtual auto-edge scroll must occur"
    );
    if takeover {
        assert_eq!(app.large.split_scroll.sequence, sequence + 1);
        assert_eq!(app.large.split_scroll.driver, Some(Pane::Source));
        let pos = app.large.split_scroll.views[1].rect.unwrap().center();
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            60,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -40.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert_eq!(app.large.split_scroll.driver, Some(Pane::Preview));
        for _ in 0..4 {
            render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
        }
        assert_eq!(app.large.split_scroll.driver, Some(Pane::Preview));
        assert_eq!(app.large.split_scroll.sequence, sequence + 2);
    } else {
        assert!(
            (after[1] - before[1]).abs() <= 1.,
            "selection auto-scroll must not return to preview"
        );
        assert_eq!(app.large.split_scroll.sequence, sequence);
        assert_eq!(app.large.split_scroll.driver, None);
    }
    assert_ne!(app.large.selection, selection);
    assert_eq!(app.doc.text, text);
    assert_eq!(app.doc.revision(), revision);
    assert_eq!(app.doc.dirty(), dirty);
}

#[test]
fn virtual_ime_commit_keeps_its_window_coordinates_after_preview_hover_and_scroll() {
    for wheel in [false, true] {
        let ctx = egui::Context::default();
        let text = (0..30000)
            .map(|i| format!("Paragraph {i:06} 中文内容 scrolling.\n\n"))
            .collect::<String>();
        let mut app = crate::preview_layout_tests::editor(&ctx, &text);
        let mut frame = 0;
        app.reading.bind(&mut app.doc, "main", 0);
        for _ in 0..400 {
            render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
            if app.large.split_scroll.current && !app.large.split_scroll.views[1].galleys.is_empty()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let viewport = app.large.split_scroll.views[0].rect.unwrap();
        let pos = egui::pos2(viewport.left() + 80., viewport.top() + 100.);
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            60,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            60,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let caret = app.large.selection[0];
        let byte = text
            .char_indices()
            .nth(caret)
            .map_or(text.len(), |(b, _)| b);
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            60,
            vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit("ni'hao".into())),
            ],
        );
        assert!(app.doc.text.contains("ni'hao"));
        for _ in 0..400 {
            render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
            if app.large.split_scroll.current {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            app.large.split_scroll.current,
            "preedit snapshot must be current before exercising native preview following"
        );
        let selection = app.large.selection;
        let focus = ctx.memory(|m| m.focused());
        let before = app.large.split_scroll.views[0].offset;
        let pos = app.large.split_scroll.views[1].rect.unwrap().center();
        let mut events = vec![egui::Event::PointerMoved(pos)];
        if wheel {
            events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -40.),
                modifiers: egui::Modifiers::NONE,
            });
        }
        render_native_fps(&mut app, &ctx, &mut frame, 60, events);
        for _ in 0..16 {
            render_native_fps(&mut app, &ctx, &mut frame, 60, vec![]);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(app.large.selection, selection);
        assert_eq!(ctx.memory(|m| m.focused()), focus);
        if wheel {
            assert!(
                app.large.split_scroll.views[0].offset > before + 1.,
                "must exercise a moved virtual source viewport"
            );
        }
        render_native_fps(
            &mut app,
            &ctx,
            &mut frame,
            60,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("你好".into()))],
        );
        let mut expected = text.clone();
        expected.insert_str(byte, "你好");
        eprintln!(
            "VIRTUAL_IME wheel={wheel} caret={caret} viewport_before={before} after={} raw_pinyin={} chinese={}",
            app.large.split_scroll.views[0].offset,
            app.doc.text.matches("ni'hao").count(),
            app.doc.text.matches("你好").count()
        );
        assert!(
            app.doc.text == expected,
            "commit must replace preedit at its original global source coordinate (wheel={wheel}); actual raw/Chinese counts={}/{}",
            app.doc.text.matches("ni'hao").count(),
            app.doc.text.matches("你好").count()
        );
        assert_eq!(app.large.selection, [caret + 2, caret + 2]);
    }
}
