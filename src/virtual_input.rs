// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit local acceptance transport. This injects egui input, never scroll state.
use crate::egui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, BufRead, Write},
    sync::{Arc, Condvar, Mutex, mpsc},
    time::Instant,
};

const MAX_LINE: usize = 16 * 1024;
const MAX_EVENTS: usize = 32;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Session {
    pub buffer_id: u64,
    pub generation: u64,
    pub revision: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    seq: u64,
    session: Option<Session>,
    #[serde(default)]
    events: Vec<Input>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    Move {
        x: f32,
        y: f32,
    },
    Wheel {
        unit: Unit,
        delta: [f32; 2],
    },
    Button {
        pressed: bool,
    },
    Key {
        key: Key,
        pressed: bool,
        #[serde(default)]
        shift: bool,
    },
    Text {
        text: String,
    },
    Undo,
    Redo,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Unit {
    Point,
    Line,
}

#[derive(Clone, Copy, Deserialize)]
enum Key {
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    Escape,
}
impl Key {
    fn egui(self) -> egui::Key {
        match self {
            Self::PageUp => egui::Key::PageUp,
            Self::PageDown => egui::Key::PageDown,
            Self::ArrowUp => egui::Key::ArrowUp,
            Self::ArrowDown => egui::Key::ArrowDown,
            Self::ArrowLeft => egui::Key::ArrowLeft,
            Self::ArrowRight => egui::Key::ArrowRight,
            Self::Home => egui::Key::Home,
            Self::End => egui::Key::End,
            Self::Escape => egui::Key::Escape,
        }
    }
}

#[derive(Default)]
struct Held {
    pointer: Option<egui::Pos2>,
    button: bool,
    keys: Vec<egui::Key>,
}
impl Held {
    fn release(&mut self, events: &mut Vec<egui::Event>) {
        for key in self.keys.drain(..) {
            events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        if self.pointer.is_some() {
            events.push(egui::Event::PointerGone);
        }
        *self = Self::default();
    }
    fn append(
        &mut self,
        inputs: &[Input],
        events: &mut Vec<egui::Event>,
    ) -> Result<(), &'static str> {
        if inputs.len() > MAX_EVENTS {
            return Err("event_limit");
        }
        // Validate the complete batch before any event reaches egui.
        let mut pointer = self.pointer;
        for input in inputs {
            match input {
                Input::Move { x, y }
                    if x.is_finite()
                        && y.is_finite()
                        && (0. ..=16384.).contains(x)
                        && (0. ..=16384.).contains(y) =>
                {
                    pointer = Some(egui::pos2(*x, *y))
                }
                Input::Wheel { delta, .. }
                    if pointer.is_some()
                        && delta.iter().all(|d| d.is_finite() && d.abs() <= 4096.) => {}
                Input::Button { .. } if pointer.is_some() => {}
                Input::Key { .. } => {}
                Input::Text { text } if text.len() <= 4096 && !text.contains('\0') => {}
                Input::Undo | Input::Redo => {}
                _ => return Err("invalid_event"),
            }
        }
        for input in inputs {
            let event = match *input {
                Input::Move { x, y } => {
                    let p = egui::pos2(x, y);
                    self.pointer = Some(p);
                    egui::Event::PointerMoved(p)
                }
                Input::Wheel { ref unit, delta } => egui::Event::MouseWheel {
                    unit: match unit {
                        Unit::Point => egui::MouseWheelUnit::Point,
                        Unit::Line => egui::MouseWheelUnit::Line,
                    },
                    delta: egui::vec2(delta[0], delta[1]),
                    modifiers: egui::Modifiers::NONE,
                },
                Input::Button { pressed } => {
                    self.button = pressed;
                    egui::Event::PointerButton {
                        pos: self.pointer.unwrap(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }
                }
                Input::Key {
                    key,
                    pressed,
                    shift,
                } => {
                    let key = key.egui();
                    let repeat = pressed && self.keys.contains(&key);
                    if pressed && !repeat {
                        self.keys.push(key);
                    } else if !pressed {
                        self.keys.retain(|k| *k != key);
                    }
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat,
                        modifiers: egui::Modifiers {
                            shift,
                            ..egui::Modifiers::NONE
                        },
                    }
                }
                Input::Text { ref text } => egui::Event::Text(text.clone()),
                Input::Undo | Input::Redo => {
                    let key = if matches!(input, Input::Undo) {
                        egui::Key::Z
                    } else {
                        egui::Key::Y
                    };
                    let modifiers = egui::Modifiers {
                        ctrl: true,
                        command: true,
                        ..egui::Modifiers::NONE
                    };
                    events.push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    });
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers,
                    }
                }
            };
            events.push(event);
        }
        Ok(())
    }
}

fn read_line(reader: &mut impl BufRead) -> io::Result<Option<Result<Vec<u8>, &'static str>>> {
    let mut line = Vec::new();
    let mut oversized = false;
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok((!line.is_empty() || oversized).then_some(Err("unterminated_line")));
        }
        let end = buf.iter().position(|b| *b == b'\n');
        let count = end.map_or(buf.len(), |n| n + 1);
        if !oversized && line.len() + count <= MAX_LINE {
            line.extend_from_slice(&buf[..count]);
        } else {
            oversized = true;
            line.clear();
        }
        reader.consume(count);
        if end.is_some() {
            return Ok(Some(if oversized {
                Err("line_limit")
            } else {
                Ok(line)
            }));
        }
    }
}

struct Pending {
    request: Result<Request, &'static str>,
    received: Value,
    reply: mpsc::SyncSender<Value>,
}

#[derive(Default)]
struct OutputState {
    acks: VecDeque<Value>,
    latest: Option<Value>,
    paint: Option<Value>,
    replaced: u64,
    closed: bool,
}
#[derive(Default)]
struct Output {
    state: Mutex<OutputState>,
    wake: Condvar,
}
impl Output {
    fn ack(&self, ack: Value) {
        let mut s = self.state.lock().unwrap();
        while s.acks.len() >= 4 && !s.closed {
            s = self.wake.wait(s).unwrap();
        }
        if !s.closed {
            s.acks.push_back(ack);
            self.wake.notify_one();
        }
    }
    fn latest(&self, mut state: Value) {
        let mut s = self.state.lock().unwrap();
        if s.closed {
            return;
        }
        if s.latest.is_some() {
            s.replaced += 1;
        }
        state["state_replaced"] = s.replaced.into();
        s.latest = Some(state);
        self.wake.notify_one();
    }
    fn paint(&self, state: Value) {
        let mut s = self.state.lock().unwrap();
        if !s.closed {
            s.paint = Some(state);
            self.wake.notify_one();
        }
    }
    fn write(&self, mut writer: impl Write) {
        loop {
            let next = {
                let mut s = self.state.lock().unwrap();
                while s.acks.is_empty() && s.latest.is_none() && s.paint.is_none() && !s.closed {
                    s = self.wake.wait(s).unwrap();
                }
                if s.closed {
                    return;
                }
                let next = s
                    .acks
                    .pop_front()
                    .or_else(|| s.latest.take())
                    .or_else(|| s.paint.take())
                    .unwrap();
                self.wake.notify_all();
                next
            };
            if serde_json::to_writer(&mut writer, &next)
                .map_err(io::Error::other)
                .and_then(|_| writer.write_all(b"\n"))
                .and_then(|_| writer.flush())
                .is_err()
            {
                self.close();
                return;
            }
        }
    }
    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.wake.notify_all();
    }
}

fn stamp(start: Instant) -> Value {
    #[cfg(windows)]
    let qpc = {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn QueryPerformanceCounter(value: *mut i64) -> i32;
            fn QueryPerformanceFrequency(value: *mut i64) -> i32;
        }
        let (mut ticks, mut frequency) = (0i64, 0i64);
        // SAFETY: these APIs write one valid stack-allocated i64 each.
        if unsafe {
            QueryPerformanceCounter(&mut ticks) != 0
                && QueryPerformanceFrequency(&mut frequency) != 0
        } {
            json!({"ticks":ticks,"frequency":frequency})
        } else {
            Value::Null
        }
    };
    #[cfg(not(windows))]
    let qpc = Value::Null;
    json!({"elapsed_us":start.elapsed().as_micros() as u64,"qpc":qpc})
}

pub(crate) struct VirtualInput {
    receiver: mpsc::Receiver<Pending>,
    output: Arc<Output>,
    start: Instant,
    held: Held,
    session: Option<Session>,
    last_seq: Option<u64>,
    accepted_seq: Option<u64>,
}
impl Drop for VirtualInput {
    fn drop(&mut self) {
        self.output.close();
    }
}
impl VirtualInput {
    pub fn from_environment(ctx: &egui::Context) -> Option<Self> {
        if !cfg!(windows)
            || cfg!(test)
            || std::env::var_os("MDCARD_VIRTUAL_INPUT").is_none_or(|v| v != "1")
        {
            return None;
        }
        let (sender, receiver) = mpsc::sync_channel(4);
        let output = Arc::new(Output::default());
        let start = Instant::now();
        let writer = output.clone();
        std::thread::Builder::new()
            .name("mdcard-virtual-output".into())
            .spawn(move || writer.write(io::stdout().lock()))
            .ok()?;
        let writer = output.clone();
        let ctx = ctx.clone();
        if std::thread::Builder::new()
            .name("mdcard-virtual-input".into())
            .spawn(move || {
                let mut input = io::stdin().lock();
                loop {
                    let request = match read_line(&mut input) {
                        Ok(Some(Ok(line))) => {
                            serde_json::from_slice::<Request>(&line).map_err(|_| "invalid_json")
                        }
                        Ok(Some(Err(reason))) => Err(reason),
                        Ok(None) => Err("eof"),
                        Err(_) => Err("read_error"),
                    };
                    let done = matches!(request, Err("eof" | "read_error"));
                    let (reply, ack) = mpsc::sync_channel(1);
                    if sender
                        .send(Pending {
                            request,
                            received: stamp(start),
                            reply,
                        })
                        .is_err()
                    {
                        break;
                    }
                    ctx.request_repaint();
                    match ack.recv() {
                        Ok(ack) => writer.ack(ack),
                        Err(_) => break,
                    }
                    if done {
                        break;
                    }
                }
            })
            .is_err()
        {
            output.close();
            return None;
        }
        Some(Self {
            receiver,
            output,
            start,
            held: Held::default(),
            session: None,
            last_seq: None,
            accepted_seq: None,
        })
    }
    pub fn hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput, session: Session) {
        // Test context only: this changes egui focus, never OS focus/activation.
        raw.events
            .retain(|e| matches!(e, egui::Event::Screenshot { .. }));
        raw.focused = true;
        raw.modifiers = egui::Modifiers::NONE;
        raw.hovered_files.clear();
        raw.dropped_files.clear();
        if self.session != Some(session) {
            self.cancel(ctx, &mut raw.events);
            self.session = Some(session);
        }
        if let Ok(pending) = self.receiver.try_recv() {
            let seq = pending.request.as_ref().ok().map(|r| r.seq);
            let result = match pending.request {
                Err(reason) => Err(reason),
                Ok(request) => {
                    if self.last_seq.is_some_and(|old| request.seq <= old) {
                        Err("non_monotonic_seq")
                    } else {
                        self.last_seq = Some(request.seq);
                        if !request.events.is_empty() && request.session != Some(session) {
                            Err("stale_session")
                        } else {
                            self.held
                                .append(&request.events, &mut raw.events)
                                .map(|()| {
                                    if !request.events.is_empty() {
                                        self.accepted_seq = Some(request.seq);
                                    }
                                })
                        }
                    }
                }
            };
            if result.is_err() {
                self.cancel(ctx, &mut raw.events);
            }
            let ack = json!({"kind":"ack","seq":seq,"accepted":result.is_ok(),"reason":result.err(),"session":session,"received":pending.received,"consumed":stamp(self.start),"frame":ctx.cumulative_frame_nr(),"timing_boundary":"raw_input_hook"});
            let _ = pending.reply.try_send(ack);
        }
    }
    fn cancel(&mut self, ctx: &egui::Context, events: &mut Vec<egui::Event>) {
        // A synthetic button release could click a control in the new session.
        // Cancel egui's old gesture without generating a release/click event.
        ctx.input_mut(|i| i.pointer = Default::default());
        ctx.stop_dragging();
        self.held.release(events);
        self.accepted_seq = None;
    }
    pub fn publish(&self, ctx: &egui::Context, session: Session, state: Value) {
        self.output.latest(json!({"kind":"state","session":session,"frame":ctx.cumulative_frame_nr(),"accepted_seq":self.accepted_seq,"at":stamp(self.start),"timing_boundary":"ui_geometry_complete","state":state}));
    }
    pub fn paint_callback(&self, ctx: &egui::Context, session: Session) {
        let output = self.output.clone();
        let start = self.start;
        let frame = ctx.cumulative_frame_nr();
        let seq = self.accepted_seq;
        let callback = eframe::egui_glow::CallbackFn::new(move |_, _| {
            output.paint(json!({"kind":"paint","session":session,"frame":frame,"accepted_seq":seq,"at":stamp(start),"timing_boundary":"gl_callback_no_finish"}));
        });
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("virtual-input-paint"),
        ))
        .add(egui::Shape::Callback(egui::PaintCallback {
            rect: ctx.content_rect(),
            callback: Arc::new(callback),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_lines_and_batches_reject_oversize_and_partial_input() {
        assert!(matches!(read_line(&mut &b"{}\n"[..]).unwrap(), Some(Ok(_))));
        assert!(matches!(
            read_line(&mut &b"{}"[..]).unwrap(),
            Some(Err("unterminated_line"))
        ));
        assert!(matches!(
            read_line(&mut vec![b'a'; MAX_LINE + 1].as_slice()).unwrap(),
            Some(Err(_))
        ));
        let mut held = Held::default();
        let mut events = vec![];
        let inputs = (0..33)
            .map(|_| Input::Move { x: 1., y: 2. })
            .collect::<Vec<_>>();
        assert_eq!(held.append(&inputs, &mut events), Err("event_limit"));
        assert!(events.is_empty());
        assert_eq!(
            held.append(
                &[
                    Input::Move { x: 1., y: 2. },
                    Input::Wheel {
                        unit: Unit::Point,
                        delta: [0., f32::NAN]
                    }
                ],
                &mut events
            ),
            Err("invalid_event")
        );
        assert!(held.pointer.is_none());
    }
    #[test]
    fn eof_invalid_and_old_session_release_held_state_and_native_gone_cannot_cancel_pointer() {
        let ctx = egui::Context::default();
        let (tx, receiver) = mpsc::sync_channel(4);
        let mut v = VirtualInput {
            receiver,
            output: Arc::default(),
            start: Instant::now(),
            held: Held::default(),
            session: None,
            last_seq: None,
            accepted_seq: None,
        };
        let session = Session {
            buffer_id: 1,
            generation: 2,
            revision: 3,
        };
        let run = |v: &mut VirtualInput, request, raw: &mut egui::RawInput| {
            let (reply, rx) = mpsc::sync_channel(1);
            tx.send(Pending {
                request,
                received: Value::Null,
                reply,
            })
            .unwrap();
            v.hook(&ctx, raw, session);
            rx.recv().unwrap()
        };
        let mut raw = egui::RawInput::default();
        let request=serde_json::from_value(json!({"seq":1,"session":session,"events":[{"type":"move","x":50,"y":60},{"type":"button","pressed":true},{"type":"key","key":"PageDown","pressed":true}]})).unwrap();
        assert_eq!(run(&mut v, Ok(request), &mut raw)["accepted"], true);
        assert!(v.held.button);
        raw.events = vec![egui::Event::PointerGone];
        v.hook(&ctx, &mut raw, session);
        assert!(raw.events.is_empty());
        assert!(v.held.pointer.is_some());
        raw.events.clear();
        let stale = Request {
            seq: 2,
            session: Some(Session {
                revision: 2,
                ..session
            }),
            events: vec![Input::Button { pressed: false }],
        };
        assert_eq!(run(&mut v, Ok(stale), &mut raw)["reason"], "stale_session");
        assert!(!v.held.button);
        assert!(v.held.keys.is_empty());
        assert!(
            !raw.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerButton { pressed: false, .. }))
        );
        let rearm = Request {
            seq: 3,
            session: Some(session),
            events: vec![
                Input::Move { x: 50., y: 60. },
                Input::Button { pressed: true },
            ],
        };
        raw.events.clear();
        assert_eq!(run(&mut v, Ok(rearm), &mut raw)["accepted"], true);
        assert!(v.held.button);
        raw.events.clear();
        assert_eq!(run(&mut v, Err("eof"), &mut raw)["reason"], "eof");
        assert!(!v.held.button);
        assert!(
            raw.events
                .iter()
                .all(|e| !matches!(e, egui::Event::PointerButton { .. }))
        );
        let old = Request {
            seq: 1,
            session: Some(session),
            events: vec![],
        };
        assert_eq!(
            run(&mut v, Ok(old), &mut raw)["reason"],
            "non_monotonic_seq"
        );
    }
    #[test]
    fn ordinary_constructor_creates_no_transport() {
        assert!(VirtualInput::from_environment(&egui::Context::default()).is_none());
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "ordinary");
        let mut raw = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(egui::pos2(40., 50.)),
                egui::Event::Text("normal".into()),
            ],
            focused: false,
            ..Default::default()
        };
        use eframe::App;
        app.raw_input_hook(&ctx, &mut raw);
        assert!(!raw.focused);
        assert_eq!(raw.events.len(), 2);
        assert!(matches!(&raw.events[1],egui::Event::Text(s) if s=="normal"));
    }
    #[test]
    fn changed_session_cancels_real_egui_press_without_clicking_new_content() {
        let ctx = egui::Context::default();
        let (_tx, receiver) = mpsc::sync_channel(4);
        let old = Session {
            buffer_id: 1,
            generation: 1,
            revision: 0,
        };
        let mut v = VirtualInput {
            receiver,
            output: Arc::default(),
            start: Instant::now(),
            held: Held::default(),
            session: Some(old),
            last_seq: None,
            accepted_seq: None,
        };
        let mut button = egui::Rect::NOTHING;
        let mut clicked = false;
        let raw = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600., 400.),
            )),
            focused: true,
            ..Default::default()
        };
        let _ = ctx.run(raw(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = ui.button("same widget in a new document");
                button = response.rect;
                clicked |= response.clicked();
            });
        });
        let mut press = raw();
        v.held
            .append(
                &[
                    Input::Move {
                        x: button.center().x,
                        y: button.center().y,
                    },
                    Input::Button { pressed: true },
                ],
                &mut press.events,
            )
            .unwrap();
        let _ = ctx.run(press, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(
                    ui.button("same widget in a new document")
                        .is_pointer_button_down_on()
                );
            });
        });
        assert!(ctx.input(|i| i.pointer.primary_down()));
        let mut next = raw();
        v.hook(
            &ctx,
            &mut next,
            Session {
                generation: 2,
                ..old
            },
        );
        let _ = ctx.run(next, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let r = ui.button("same widget in a new document");
                clicked |= r.clicked();
                assert!(!r.dragged());
            });
        });
        assert!(
            !clicked,
            "a stale session cancellation must not commit a click in its replacement"
        );
        assert!(!ctx.input(|i| i.pointer.primary_down()));
    }
    #[test]
    fn fixture_text_and_undo_redo_are_bounded_original_editor_events() {
        let mut held = Held::default();
        let mut events = vec![];
        assert_eq!(
            held.append(
                &[Input::Text {
                    text: "x".repeat(4097)
                }],
                &mut events
            ),
            Err("invalid_event")
        );
        held.append(
            &[
                Input::Text {
                    text: "fixture".into(),
                },
                Input::Undo,
                Input::Redo,
            ],
            &mut events,
        )
        .unwrap();
        assert!(matches!(&events[0],egui::Event::Text(t) if t=="fixture"));
        assert!(
            matches!(&events[1],egui::Event::Key{key:egui::Key::Z,pressed:true,modifiers,..} if modifiers.ctrl&&modifiers.command)
        );
        assert!(matches!(
            &events[3],
            egui::Event::Key {
                key: egui::Key::Y,
                pressed: true,
                ..
            }
        ));
        let output = Output::default();
        output.latest(json!({"kind":"state"}));
        output.paint(json!({"kind":"paint"}));
        let s = output.state.lock().unwrap();
        assert!(
            s.latest.is_some() && s.paint.is_some(),
            "paint must not overwrite the latest UI state"
        );
    }
}
