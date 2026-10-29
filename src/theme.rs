// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use eframe::egui::{self, Color32, FontId, Response, RichText, Stroke, Vec2};
/// Shared logical-pixel rhythm. Panel-specific document layout stays separate.
pub const CONTROL_HEIGHT: f32 = 32.0;
pub const SIDEBAR_WIDTH: f32 = 244.0;
pub const CONTENT_MARGIN: f32 = 24.0;
pub const SPLIT_CONTENT_MIN: f32 = 860.0;
pub fn window_bounds(ctx: &egui::Context) -> egui::Rect {
    ctx.content_rect().shrink(12.0)
}
pub fn window_title(text: &str) -> RichText {
    RichText::new(text).size(16.0).strong()
}
pub const ACCENTS: [&str; 6] = ["Blue", "Purple", "Pink", "Orange", "Green", "Graphite"];
#[derive(Clone, Copy)]
pub struct Palette {
    pub ink: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub paper: Color32,
    pub sidebar: Color32,
    pub chrome: Color32,
    pub line: Color32,
    pub selected: Color32,
    pub control: Color32,
    pub dark: bool,
}
fn rgb(v: [u8; 3]) -> Color32 {
    Color32::from_rgb(v[0], v[1], v[2])
}
pub(crate) fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let a = a.to_array();
    let b = b.to_array();
    Color32::from_rgba_unmultiplied(
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
        (a[3] as f32 + (b[3] as f32 - a[3] as f32) * t) as u8,
    )
}
impl Palette {
    pub fn new(dark: bool, accent: usize) -> Self {
        let colors = if dark {
            [
                [145, 177, 250],
                [192, 152, 255],
                [255, 138, 189],
                [255, 182, 112],
                [111, 205, 155],
                [190, 197, 211],
            ]
        } else {
            [
                [58, 86, 157],
                [120, 68, 183],
                [183, 46, 110],
                [165, 83, 17],
                [33, 120, 73],
                [85, 94, 112],
            ]
        };
        let paper = rgb(if dark { [31, 36, 45] } else { [252, 252, 253] });
        let accent = rgb(colors[accent.min(5)]);
        Self {
            ink: rgb(if dark { [228, 233, 241] } else { [41, 51, 66] }),
            muted: rgb(if dark {
                [171, 182, 199]
            } else {
                [89, 102, 120]
            }),
            accent,
            paper,
            sidebar: rgb(if dark { [37, 43, 54] } else { [240, 243, 247] }),
            chrome: rgb(if dark { [42, 49, 64] } else { [233, 237, 243] }),
            line: rgb(if dark { [69, 79, 97] } else { [214, 221, 231] }),
            selected: mix(paper, accent, if dark { 0.18 } else { 0.11 }),
            control: rgb(if dark { [49, 58, 73] } else { [245, 247, 250] }),
            dark,
        }
    }
    pub fn blend(self, other: Self, t: f32) -> Self {
        Self {
            ink: mix(self.ink, other.ink, t),
            muted: mix(self.muted, other.muted, t),
            accent: mix(self.accent, other.accent, t),
            paper: mix(self.paper, other.paper, t),
            sidebar: mix(self.sidebar, other.sidebar, t),
            chrome: mix(self.chrome, other.chrome, t),
            line: mix(self.line, other.line, t),
            selected: mix(self.selected, other.selected, t),
            control: mix(self.control, other.control, t),
            dark: other.dark,
        }
    }
    pub fn apply(self, ctx: &egui::Context, reduce_motion: bool) {
        ctx.style_mut(|style| {
            let mut v = if self.dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            v.panel_fill = self.paper;
            v.window_fill = self.paper;
            v.extreme_bg_color = self.paper;
            v.faint_bg_color = self.sidebar;
            v.code_bg_color = self.sidebar;
            v.override_text_color = Some(self.ink);
            v.weak_text_color = Some(self.muted);
            v.hyperlink_color = self.accent;
            v.selection.bg_fill = self.selected;
            v.selection.stroke = Stroke::new(1.0, self.accent);
            v.window_stroke = Stroke::new(1.0, self.line);
            v.window_corner_radius = 10.into();
            v.menu_corner_radius = 6.into();
            v.window_highlight_topmost = false;
            v.window_shadow = egui::epaint::Shadow {
                offset: [0, 5],
                blur: 16,
                spread: 0,
                color: Color32::from_black_alpha(if self.dark { 72 } else { 24 }),
            };
            for w in [
                &mut v.widgets.noninteractive,
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
                &mut v.widgets.open,
            ] {
                w.corner_radius = 5.into();
                w.bg_stroke = Stroke::new(1.0, self.line);
                w.fg_stroke = Stroke::new(1.0, self.ink);
            }
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.line);
            v.widgets.noninteractive.bg_fill = self.chrome;
            v.widgets.inactive.bg_fill = self.control;
            v.widgets.inactive.weak_bg_fill = self.control;
            v.widgets.hovered.bg_fill = self.selected;
            v.widgets.hovered.weak_bg_fill = self.selected;
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, self.accent);
            v.widgets.active.bg_fill = self.selected;
            v.widgets.active.weak_bg_fill = self.selected;
            v.widgets.active.bg_stroke = Stroke::new(1.5, self.accent);
            v.widgets.open.bg_fill = self.selected;
            v.widgets.open.weak_bg_fill = self.selected;
            style.visuals = v;
            style.animation_time = if reduce_motion { 0.0 } else { 0.16 };
            style.spacing.interact_size = Vec2::new(32.0, CONTROL_HEIGHT);
            style.spacing.item_spacing = Vec2::new(8.0, 8.0);
            style.spacing.button_padding = Vec2::new(12.0, 6.0);
            style.spacing.window_margin = egui::Margin::symmetric(18, 10);
            style.spacing.menu_margin = egui::Margin::same(8);
            style.spacing.scroll.foreground_color = false;
            style.spacing.scroll.bar_width = 6.0;
            style
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(13.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, FontId::proportional(12.0));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(27.0));
        });
    }
}
pub fn button(ui: &mut egui::Ui, text: impl Into<String>) -> Response {
    // Native Button supplies activation, disabled rendering, wrapping and focus
    // semantics consistently in toolbars, menus and modal action rows.
    let text = text.into();
    let response = ui.add(
        egui::Button::new(&text)
            .min_size(Vec2::new(0.0, CONTROL_HEIGHT))
            .wrap(),
    );
    record_control(ui, &response, &text);
    response
}

pub fn primary_button(ui: &mut egui::Ui, text: &str, p: Palette) -> Response {
    let foreground = if p.dark {
        Color32::from_rgb(24, 34, 49)
    } else {
        Color32::WHITE
    };
    let response = ui.add(
        egui::Button::new(RichText::new(text).color(foreground))
            .fill(p.accent)
            .min_size(Vec2::new(0.0, CONTROL_HEIGHT)),
    );
    record_control(ui, &response, text);
    response
}

/// Reserve a readable action row, wrapping only when the real labels need it.
pub fn action_row<R>(
    ui: &mut egui::Ui,
    labels: &[&str],
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let width = labels
            .iter()
            .map(|label| {
                ui.painter()
                    .layout_no_wrap(
                        (*label).into(),
                        FontId::proportional(13.0),
                        ui.visuals().text_color(),
                    )
                    .size()
                    .x
                    + ui.spacing().button_padding.x * 2.0
            })
            .sum::<f32>()
            + ui.spacing().item_spacing.x * labels.len().saturating_sub(1) as f32;
        ui.add_space((ui.available_width() - width).max(0.0));
        contents(ui)
    })
}

pub fn search_button(ui: &mut egui::Ui, text: &str, width: f32, p: Palette) -> Response {
    let response = ui.add_sized(
        [width, CONTROL_HEIGHT],
        egui::Button::new("")
            .fill(p.sidebar)
            .stroke(Stroke::new(0.5, p.line)),
    );
    let c = egui::pos2(response.rect.left() + 15.0, response.rect.center().y - 1.0);
    let stroke = Stroke::new(1.4, p.muted);
    ui.painter().circle_stroke(c, 4.5, stroke);
    ui.painter()
        .line_segment([c + Vec2::new(3.5, 3.5), c + Vec2::new(7.0, 7.0)], stroke);
    let galley = egui::WidgetText::from(RichText::new(text).size(12.0).color(p.muted)).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (width - 40.0).max(20.0),
        FontId::proportional(12.0),
    );
    ui.painter().galley(
        egui::pos2(
            response.rect.left() + 33.0,
            response.rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        p.muted,
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text));
    record_control(ui, &response, text);
    response
}

/// Flat menu chrome; popup rows keep native activation and keyboard semantics.
pub fn menu_button<R>(
    ui: &mut egui::Ui,
    title: &str,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    ui.scope(|ui| {
        let inactive = &mut ui.visuals_mut().widgets.inactive;
        inactive.bg_fill = Color32::TRANSPARENT;
        inactive.weak_bg_fill = Color32::TRANSPARENT;
        inactive.bg_stroke = Stroke::NONE;
        ui.menu_button(title, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            contents(ui)
        })
    })
    .inner
}

pub fn navigation_tab(
    ui: &mut egui::Ui,
    text: &str,
    selected: bool,
    width: f32,
    p: Palette,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 32.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(0.0, 2.0)), 4, p.selected);
    }
    if selected {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(2.0, p.accent),
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.0),
        if selected { p.accent } else { p.muted },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            4,
            Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::RadioButton,
            ui.is_enabled(),
            selected,
            text,
        )
    });
    response
}

pub fn pane_toolbar<R>(
    ui: &mut egui::Ui,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 28.0;
        ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
        ui.horizontal_wrapped(|ui| {
            ui.set_min_height(32.0);
            contents(ui)
        })
    })
    .inner
}

pub fn tool_button(ui: &mut egui::Ui, text: &str) -> Response {
    let response = ui.add(
        egui::Button::new(RichText::new(text).size(12.0))
            .frame(false)
            .min_size(Vec2::new(0.0, 28.0)),
    );
    record_control(ui, &response, text);
    response
}

/// Shared document identity row. Focus is supplied by the real editor owner;
/// opening a dialog therefore never changes the document or its undo history.
pub fn document_header(
    ui: &mut egui::Ui,
    name: &str,
    path: &str,
    state: &str,
    flags: (bool, bool),
    p: Palette,
) {
    let (active, dirty) = flags;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 52.0), egui::Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            ui.is_enabled(),
            format!("{name}\n{path}\n{state}"),
        )
    });
    let state_width = (width * 0.32).clamp(60.0, 260.0);
    let left = rect.left() + 12.0;
    let title = if dirty {
        format!("●  {name}")
    } else {
        name.into()
    };
    let title = egui::WidgetText::from(RichText::new(title).size(15.0).strong()).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (width - state_width - 24.0).max(20.0),
        FontId::proportional(15.0),
    );
    ui.painter()
        .galley(egui::pos2(left, rect.top() + 2.0), title, p.ink);
    let path_text = egui::WidgetText::from(RichText::new(path).size(11.0).color(p.muted))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            (width - 24.0).max(20.0),
            FontId::proportional(11.0),
        );
    ui.painter()
        .galley(egui::pos2(left, rect.top() + 27.0), path_text, p.muted);
    let state_text = egui::WidgetText::from(RichText::new(state).size(11.0).color(p.muted))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            state_width,
            FontId::proportional(11.0),
        );
    ui.painter().galley(
        egui::pos2(rect.right() - state_text.size().x, rect.top() + 6.0),
        state_text,
        p.muted,
    );
    if active {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 4.0), Vec2::new(3.0, 28.0)),
            2,
            p.accent,
        );
    }
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.5, p.line),
    );
    response.on_hover_text(format!("{name}\n{path}\n{state}"));
}

pub fn record_control(ui: &egui::Ui, response: &Response, name: &str) {
    if std::env::var_os("MDCARD_UI_REPORT").is_none() {
        return;
    }
    let r = response.rect;
    let clip = ui.clip_rect();
    let value = serde_json::json!({"name":name,"rect":[r.min.x,r.min.y,r.max.x,r.max.y],"clip":[clip.min.x,clip.min.y,clip.max.x,clip.max.y],"enabled":response.enabled(),"focus":response.has_focus(),"hovered":response.hovered(),"layer":format!("{:?}",response.layer_id)});
    ui.ctx().data_mut(|d| {
        let id = egui::Id::new("ui-report-controls");
        let mut rows = d.get_temp::<Vec<serde_json::Value>>(id).unwrap_or_default();
        rows.push(value);
        d.insert_temp(id, rows);
    });
}

/// Record the actual trigger before a modal takes focus. Restore on a later
/// frame because egui still owns the closing modal layer in the closing frame.
pub fn begin_modal_focus(ctx: &egui::Context) {
    let (was_open, pending) = ctx.data_mut(|d| {
        (
            d.get_temp::<bool>(egui::Id::new("ui-modal-open"))
                .unwrap_or(false),
            {
                let id = egui::Id::new("ui-return-focus-pending");
                let value = d.get_temp::<egui::Id>(id);
                d.remove::<egui::Id>(id);
                value
            },
        )
    });
    if let Some(id) = pending {
        if ctx.memory(|m| m.top_modal_layer().is_some()) {
            // The previous pass' modal still filters background focus interest.
            // Wait for that layer to clear instead of losing the request at
            // end_pass when the source widget cannot register interest yet.
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("ui-return-focus-pending"), id));
            ctx.request_repaint();
        } else {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
    if !was_open {
        let focused = ctx.memory(|m| m.focused());
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("ui-return-focus"), focused);
        });
    }
}
pub fn end_modal_focus(ctx: &egui::Context, open: bool) {
    ctx.data_mut(|d| {
        let id = egui::Id::new("ui-modal-open");
        let before = d.get_temp::<bool>(id).unwrap_or(false);
        d.insert_temp(id, open);
        if before
            && !open
            && let Some(Some(focus)) =
                d.get_temp::<Option<egui::Id>>(egui::Id::new("ui-return-focus"))
        {
            d.insert_temp(egui::Id::new("ui-return-focus-pending"), focus);
        }
    });
    if !open
        && ctx.data(|d| {
            d.get_temp::<egui::Id>(egui::Id::new("ui-return-focus-pending"))
                .is_some()
        })
    {
        ctx.request_repaint();
    }
}

#[derive(Clone, Copy)]
enum WindowFocus {
    Primary,
    Secondary,
    Widget(egui::Id),
}

/// Non-modal windows need their own bookmarks so a nested Preferences modal
/// can restore a table cell/search field without replacing the outer bookmark.
pub fn begin_window_focus(
    ctx: &egui::Context,
    key: &'static str,
    primary: egui::Id,
    secondary: Option<egui::Id>,
) {
    let was_open = ctx.data(|d| {
        d.get_temp::<bool>(egui::Id::new(("ui-window-open", key)))
            .unwrap_or(false)
    });
    let pending = ctx
        .data_mut(|d| {
            d.remove_temp::<Option<WindowFocus>>(egui::Id::new(("ui-window-focus-pending", key)))
        })
        .flatten();
    if let Some(target) = pending {
        if ctx.memory(|m| m.top_modal_layer().is_some()) {
            ctx.data_mut(|d| {
                d.insert_temp(
                    egui::Id::new(("ui-window-focus-pending", key)),
                    Some(target),
                )
            });
            ctx.request_repaint();
        } else {
            let id = match target {
                WindowFocus::Primary => primary,
                WindowFocus::Secondary => secondary.unwrap_or(primary),
                WindowFocus::Widget(id) => id,
            };
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
    if !was_open {
        let focus = ctx.memory(|m| m.focused()).map(|id| {
            if id == primary {
                WindowFocus::Primary
            } else if Some(id) == secondary {
                WindowFocus::Secondary
            } else {
                WindowFocus::Widget(id)
            }
        });
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(("ui-window-return-focus", key)), focus));
    }
}
pub fn end_window_focus(ctx: &egui::Context, key: &'static str, open: bool) {
    let closed = ctx.data_mut(|d| {
        let id = egui::Id::new(("ui-window-open", key));
        let before = d.get_temp::<bool>(id).unwrap_or(false);
        d.insert_temp(id, open);
        if before && !open {
            let focus = d
                .get_temp::<Option<WindowFocus>>(egui::Id::new(("ui-window-return-focus", key)))
                .flatten();
            d.insert_temp(egui::Id::new(("ui-window-focus-pending", key)), focus);
            focus.is_some()
        } else {
            false
        }
    });
    if closed {
        ctx.request_repaint();
    }
}
/// A result selection owns its new destination; cancellation bookmarks must
/// not steal focus from that destination on the following frame.
pub fn forget_window_focus(ctx: &egui::Context, key: &'static str) {
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new(("ui-window-return-focus", key)),
            None::<WindowFocus>,
        );
        d.remove::<Option<WindowFocus>>(egui::Id::new(("ui-window-focus-pending", key)));
    });
}
pub fn set_window_return_focus(ctx: &egui::Context, key: &'static str, id: egui::Id) {
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new(("ui-window-return-focus", key)),
            Some(WindowFocus::Widget(id)),
        )
    });
}
pub fn window_escape(ctx: &egui::Context, layer: egui::LayerId) -> bool {
    let top = ctx.memory(|m| {
        let areas = m.areas();
        // top_layer_id retains closed window IDs in its ordering; only visible
        // layers may block Escape from reaching the remaining parent window.
        let visible = m
            .layer_ids()
            .filter(|id| id.order == layer.order && areas.is_visible(id))
            .last();
        m.top_modal_layer().is_none() && visible == Some(layer)
    });
    top && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

#[derive(Clone, Copy)]
pub enum Icon {
    Sidebar,
    New,
    Open,
    Save,
    Refresh,
}

pub fn icon_button(ui: &mut egui::Ui, icon: Icon, hint: &str) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(CONTROL_HEIGHT), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    if response.hovered() || response.is_pointer_button_down_on() || response.has_focus() {
        ui.painter().rect(
            rect,
            7,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    let c = rect.center();
    let stroke = Stroke::new(1.4, visuals.fg_stroke.color);
    let line = |a: [f32; 2], b: [f32; 2]| {
        ui.painter().line_segment(
            [c + Vec2::new(a[0], a[1]), c + Vec2::new(b[0], b[1])],
            stroke,
        );
    };
    match icon {
        Icon::Sidebar => {
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(17., 14.)),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
            line([-3., -7.], [-3., 7.]);
        }
        Icon::New => {
            line([-6., 0.], [6., 0.]);
            line([0., -6.], [0., 6.]);
        }
        Icon::Open => {
            for (a, b) in [
                ([-8., 6.], [-8., -5.]),
                ([-8., -5.], [-2., -5.]),
                ([-2., -5.], [0., -2.]),
                ([0., -2.], [8., -2.]),
                ([8., -2.], [6., 6.]),
                ([6., 6.], [-8., 6.]),
            ] {
                line(a, b);
            }
        }
        Icon::Save => {
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(c, Vec2::splat(15.)),
                2,
                stroke,
                egui::StrokeKind::Inside,
            );
            for (a, b) in [
                ([-4., -7.], [-4., -1.]),
                ([-4., -1.], [4., -1.]),
                ([4., -1.], [4., -7.]),
                ([-4., 7.], [-4., 3.]),
                ([-4., 3.], [4., 3.]),
                ([4., 3.], [4., 7.]),
            ] {
                line(a, b);
            }
        }
        Icon::Refresh => {
            ui.painter().circle_stroke(c, 6., stroke);
            line([5., -6.], [6., -1.]);
            line([6., -1.], [1., -1.]);
        }
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.),
            7,
            Stroke::new(1.5, ui.visuals().hyperlink_color),
            egui::StrokeKind::Inside,
        );
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), hint));
    record_control(ui, &response, hint);
    response.on_hover_text(hint)
}
pub fn modal(ctx: &egui::Context, id: &str) -> egui::Modal {
    let size = ctx.content_rect().size();
    let resized = ctx.data_mut(|data| {
        let key = egui::Id::new(("modal-viewport-size", id));
        let previous = data.get_temp::<Vec2>(key);
        data.insert_temp(key, size);
        previous.is_some_and(|previous| previous != size)
    });
    let opacity = ctx.animate_bool_with_time(
        egui::Id::new((id, "fade")),
        true,
        ctx.style().animation_time,
    );
    egui::Modal::new(egui::Id::new(id))
        // A single invisible measuring pass drops the previous Area height on resize.
        // Keep the backdrop in the full viewport; content widths and scroll bodies
        // supply the dialog's own safe margins, independently of the backdrop.
        .area(
            egui::Modal::default_area(egui::Id::new(id))
                .constrain_to(ctx.content_rect())
                .sizing_pass(resized),
        )
        .frame(
            egui::Frame::popup(&ctx.style())
                .inner_margin(18)
                .corner_radius(10),
        )
        .backdrop_color(Color32::from_black_alpha((75.0 * opacity) as u8))
}
pub fn toggle(ui: &mut egui::Ui, value: &mut bool, text: &str) -> Response {
    let width = ui.available_width().max(120.0);
    let label = egui::WidgetText::from(RichText::new(text).size(14.0)).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (width - 54.0).max(20.0),
        FontId::proportional(14.0),
    );
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(width, 32.0), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let amount = ui.ctx().animate_bool_with_time(
        response.id.with("switch"),
        *value,
        ui.style().animation_time,
    );
    let track = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 18.0, rect.center().y),
        Vec2::new(36.0, 20.0),
    );
    let color = mix(
        ui.visuals().widgets.noninteractive.bg_stroke.color,
        ui.visuals().hyperlink_color,
        amount,
    );
    ui.painter().rect(
        track,
        10,
        color,
        Stroke::new(
            0.8,
            mix(
                ui.visuals().weak_text_color(),
                ui.visuals().hyperlink_color,
                amount,
            ),
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().circle_filled(
        egui::pos2(track.left() + 10.0 + 16.0 * amount, track.center().y),
        7.0,
        Color32::WHITE,
    );
    ui.painter().galley(
        egui::pos2(rect.left(), rect.center().y - label.size().y * 0.5),
        label,
        ui.visuals().text_color(),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            track.expand(2.0),
            13,
            Stroke::new(1.5, ui.visuals().hyperlink_color),
            egui::StrokeKind::Outside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, text)
    });
    record_control(ui, &response, text);
    response
}
pub fn label(text: &str, p: Palette) -> RichText {
    RichText::new(text).color(p.muted).size(11.0).strong()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let x = v as f32 / 255.;
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }
    fn contrast(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    #[test]
    fn readable_text_on_all_surfaces_and_accents() {
        for dark in [false, true] {
            for accent in 0..6 {
                let p = Palette::new(dark, accent);
                for bg in [p.paper, p.sidebar, p.chrome, p.control, p.selected] {
                    assert!(contrast(p.ink, bg) >= 4.5, "ink {dark} {accent}");
                    assert!(
                        contrast(p.muted, bg) >= 4.5,
                        "muted {dark} {accent} {}",
                        contrast(p.muted, bg)
                    );
                }
                assert!(contrast(p.accent, p.paper) >= 4.5, "accent {dark} {accent}");
                assert!(
                    contrast(p.accent, p.selected) >= 4.5,
                    "selected accent {dark} {accent}"
                );
            }
        }
    }
}
