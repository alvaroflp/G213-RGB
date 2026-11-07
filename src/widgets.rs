//! Hand-painted controls that follow the OpenLogi visual language.
//!
//! egui's stock widgets are usable but carry a very different visual identity.
//! Everything here paints explicitly so spacing, radii, borders and type match
//! the source design: 12 px cards, 8 px controls, one hairline border colour
//! and a single blue accent.

use crate::theme::{self, Palette, Weight};
use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, InnerResponse, Layout, Margin,
    Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

pub const CARD: CornerRadius = CornerRadius::same(theme::CARD_RADIUS);
pub const CONTROL: CornerRadius = CornerRadius::same(theme::CONTROL_RADIUS);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// A text/icon button. Sizing is measured from the label so the button never
/// truncates; `Secondary` is the outlined card-level action.
pub fn button(
    ui: &mut Ui,
    pal: &Palette,
    label: &str,
    icon: Option<&str>,
    kind: ButtonKind,
) -> Response {
    let font = theme::font(13.5, Weight::Medium);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), pal.text);
    let icon_space = if icon.is_some() { 16.0 + 7.0 } else { 0.0 };
    let width = (galley.size().x + icon_space + 24.0).max(theme::CONTROL_H);
    let (rect, response) = ui.allocate_exact_size(vec2(width, theme::CONTROL_H), Sense::click());

    let hovered = response.hovered();
    let (fill, stroke, fg) = match kind {
        ButtonKind::Primary => (
            if hovered {
                theme::blend(Color32::WHITE, theme::ACCENT, 0.12)
            } else {
                theme::ACCENT
            },
            Color32::TRANSPARENT,
            Color32::WHITE,
        ),
        ButtonKind::Secondary => (
            if hovered { pal.control } else { pal.panel },
            pal.border,
            pal.text,
        ),
        ButtonKind::Ghost => (
            if hovered { pal.control } else { Color32::TRANSPARENT },
            Color32::TRANSPARENT,
            pal.text,
        ),
        ButtonKind::Danger => (
            if hovered { pal.danger_tint() } else { Color32::TRANSPARENT },
            theme::blend(theme::STATUS_DANGER, pal.panel, 0.45),
            theme::STATUS_DANGER,
        ),
    };

    ui.painter()
        .rect(rect, CONTROL, fill, Stroke::new(1.0, stroke), StrokeKind::Inside);

    let mut x = rect.left() + 12.0;
    if let Some(icon) = icon {
        let glyph = ui.painter().text(
            pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            icon,
            FontId::proportional(16.0),
            fg,
        );
        x = glyph.right() + 7.0;
    }
    ui.painter()
        .galley(pos2(x, rect.center().y - galley.size().y * 0.5), galley, fg);

    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// Square icon-only button, used for back, close, refresh and settings.
pub fn icon_button(ui: &mut Ui, pal: &Palette, glyph: &str, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter()
            .rect_filled(rect, CONTROL, pal.control);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(18.0),
        if hovered { pal.text } else { pal.text_muted },
    );
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

/// An on/off switch, 42×24. The knob animates over ~120 ms.
pub fn toggle(ui: &mut Ui, pal: &Palette, on: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(42.0, 24.0), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, on, 0.12);
    let track = theme::blend(theme::ACCENT, pal.control, t);
    let border = theme::blend(theme::ACCENT, pal.border, t);
    ui.painter().rect(
        rect,
        CornerRadius::same(12),
        track,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let center_x = egui::lerp((rect.left() + 12.0)..=(rect.right() - 12.0), t);
    ui.painter()
        .circle_filled(pos2(center_x, rect.center().y), 9.0, Color32::WHITE);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// Title + description on the left, switch on the right. Returns the new value
/// (unchanged when the row was not clicked).
pub fn toggle_row(
    ui: &mut Ui,
    pal: &Palette,
    title: &str,
    description: &str,
    value: bool,
) -> bool {
    let mut next = value;
    let height = 42.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let toggle_rect = Rect::from_center_size(pos2(rect.right() - 21.0, rect.center().y), vec2(42.0, 24.0));
    let response = ui.interact(toggle_rect, ui.id().with(("toggle-row", title)), Sense::click());

    ui.painter().text(
        pos2(rect.left(), rect.top() + 1.0),
        Align2::LEFT_TOP,
        title,
        theme::font(14.5, Weight::Medium),
        pal.text,
    );
    ui.painter().text(
        pos2(rect.left(), rect.top() + 23.0),
        Align2::LEFT_TOP,
        description,
        theme::font(12.0, Weight::Regular),
        pal.text_muted,
    );

    if response.clicked() {
        next = !value;
    }
    // Paint the switch last so it sits above the text layer.
    let _ = toggle_at(ui, pal, toggle_rect, response, value);
    next
}

fn toggle_at(
    ui: &mut Ui,
    pal: &Palette,
    rect: Rect,
    response: Response,
    on: bool,
) -> Response {
    let t = ui.ctx().animate_bool_with_time(response.id, on, 0.12);
    let track = theme::blend(theme::ACCENT, pal.control, t);
    let border = theme::blend(theme::ACCENT, pal.border, t);
    ui.painter().rect(
        rect,
        CornerRadius::same(12),
        track,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let center_x = egui::lerp((rect.left() + 12.0)..=(rect.right() - 12.0), t);
    ui.painter()
        .circle_filled(pos2(center_x, rect.center().y), 9.0, Color32::WHITE);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A 28 px colour swatch. Selected swatches get a 2 px accent ring.
pub fn swatch(ui: &mut Ui, pal: &Palette, color: crate::config::Rgb, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let fill = theme::color32(color);
    let border = if selected {
        theme::ACCENT
    } else if response.hovered() {
        pal.text_muted
    } else {
        pal.border
    };
    let stroke = if selected { 2.0 } else { 1.0 };
    ui.painter()
        .rect(rect, CONTROL, fill, Stroke::new(stroke, border), StrokeKind::Inside);
    if response.hovered() {
        ui.painter().circle_stroke(
            rect.center(),
            16.0,
            Stroke::new(1.0, theme::blend(theme::ACCENT, pal.page, 0.5)),
        );
    }
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(color.hex())
}

/// A swatch that opens the custom colour editor.
pub fn custom_swatch(ui: &mut Ui, pal: &Palette) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    ui.painter().rect(
        rect,
        CONTROL,
        if response.hovered() {
            pal.control_hover
        } else {
            pal.control
        },
        Stroke::new(1.0, pal.border),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::PALETTE,
        FontId::proportional(15.0),
        pal.text_muted,
    );
    response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Cor personalizada")
}

/// A segmented control (Uniforme / Por zona, theme pickers, wave direction).
pub fn segmented<T: Copy + PartialEq>(
    ui: &mut Ui,
    pal: &Palette,
    id: &str,
    options: &[(T, &str)],
    selected: T,
) -> Option<T> {
    let font = theme::font(13.0, Weight::Medium);
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, label)| {
            ui.painter()
                .layout_no_wrap((*label).to_owned(), font.clone(), pal.text)
                .size()
                .x
                + 24.0
        })
        .collect();
    let total: f32 = widths.iter().sum();
    let (rect, _) = ui.allocate_exact_size(vec2(total + 4.0, 32.0), Sense::hover());
    ui.painter().rect(
        rect,
        CONTROL,
        pal.control,
        Stroke::new(1.0, pal.border),
        StrokeKind::Inside,
    );

    let mut clicked = None;
    let mut x = rect.left() + 2.0;
    for ((value, label), width) in options.iter().zip(widths) {
        let segment = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(width, 28.0));
        let response = ui.interact(segment, ui.id().with((id, *label)), Sense::click());
        let is_selected = *value == selected;
        if is_selected {
            ui.painter().rect(
                segment,
                CornerRadius::same(6),
                pal.accent_tint(),
                Stroke::new(1.0, theme::blend(theme::ACCENT, pal.panel, 0.45)),
                StrokeKind::Inside,
            );
        } else if response.hovered() {
            ui.painter()
                .rect_filled(segment, CornerRadius::same(6), pal.control_hover);
        }
        ui.painter().text(
            segment.center(),
            Align2::CENTER_CENTER,
            *label,
            font.clone(),
            if is_selected { pal.text } else { pal.text_muted },
        );
        if response.clicked() {
            clicked = Some(*value);
        }
        x += width;
    }
    clicked
}

/// A labelled slider row: title and value above a full-width track.
pub fn slider_row(
    ui: &mut Ui,
    pal: &Palette,
    label: &str,
    value: &mut u8,
    range: std::ops::RangeInclusive<u8>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.set_width(ui.available_width());
        ui.painter().text(
            pos2(ui.cursor().left(), ui.cursor().top()),
            Align2::LEFT_TOP,
            label,
            theme::font(14.0, Weight::Medium),
            pal.text,
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{value}%"))
                    .font(theme::font(12.5, Weight::Medium))
                    .color(pal.text_muted),
            );
        });
    });
    ui.add_space(2.0);
    if slider(ui, pal, value, range).changed() {
        changed = true;
    }
    changed
}

/// A custom slider. Dragging and clicking both update `value` immediately.
pub fn slider(
    ui: &mut Ui,
    pal: &Palette,
    value: &mut u8,
    range: std::ops::RangeInclusive<u8>,
) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(
        vec2(ui.available_width(), 26.0),
        Sense::click_and_drag(),
    );
    let min = *range.start();
    let max = *range.end();
    let span = f32::from(max.saturating_sub(min));
    let fraction = if span > 0.0 {
        (f32::from(value.saturating_sub(min)) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let knob_radius = if response.hovered() || response.dragged() {
        8.0
    } else {
        7.0
    };
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
    let left = track.left() + knob_radius;
    let right = track.right() - knob_radius;
    let knob_x = egui::lerp(left..=right, fraction);

    ui.painter()
        .rect_filled(track, CornerRadius::same(2), pal.control);
    if knob_x > left {
        ui.painter().rect_filled(
            Rect::from_min_max(track.min, pos2(knob_x, track.max.y)),
            CornerRadius::same(2),
            theme::ACCENT,
        );
    }
    ui.painter().circle(
        pos2(knob_x, rect.center().y),
        knob_radius,
        pal.panel,
        Stroke::new(2.0, theme::ACCENT),
    );

    if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let t = ((pointer.x - left) / (right - left)).clamp(0.0, 1.0);
        let next = min + (t * span).round() as u8;
        if next != *value {
            *value = next;
            response.mark_changed();
        }
    }
    response
}

/// A radio-card for effects. Returns the click response so the caller can
/// apply the choice.
pub fn choice_card(
    ui: &mut Ui,
    pal: &Palette,
    size: Vec2,
    icon: &str,
    title: &str,
    description: &str,
    selected: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let fill = if selected {
        pal.accent_tint()
    } else if hovered {
        pal.control_hover
    } else {
        pal.input
    };
    let border = if selected {
        theme::ACCENT
    } else {
        pal.border
    };
    ui.painter().rect(
        rect,
        CARD,
        fill,
        Stroke::new(if selected { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );

    let icon_color = if selected { theme::ACCENT } else { pal.text_muted };
    ui.painter().text(
        pos2(rect.left() + 14.0, rect.top() + 14.0),
        Align2::LEFT_TOP,
        icon,
        FontId::proportional(18.0),
        icon_color,
    );

    // Radio dot in the top-right corner.
    let dot = pos2(rect.right() - 18.0, rect.top() + 18.0);
    ui.painter().circle_stroke(
        dot,
        6.0,
        Stroke::new(
            1.5,
            if selected { theme::ACCENT } else { pal.text_muted },
        ),
    );
    if selected {
        ui.painter().circle_filled(dot, 3.0, theme::ACCENT);
    }

    ui.painter().text(
        pos2(rect.left() + 14.0, rect.top() + 42.0),
        Align2::LEFT_TOP,
        title,
        theme::font(14.5, Weight::SemiBold),
        pal.text,
    );

    let galley = ui.painter().layout(
        description.to_owned(),
        theme::font(12.0, Weight::Regular),
        pal.text_muted,
        rect.width() - 28.0,
    );
    ui.painter().galley(
        pos2(rect.left() + 14.0, rect.top() + 63.0),
        galley,
        pal.text_muted,
    );

    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A titled card. Pass `None` for an untitled surface.
pub fn panel_card<R>(
    ui: &mut Ui,
    pal: &Palette,
    title: Option<(&str, &str)>,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    Frame::new()
        .fill(pal.panel)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CARD)
        .inner_margin(Margin::same(theme::CARD_PAD as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some((icon, title)) = title {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(icon)
                            .font(FontId::proportional(16.0))
                            .color(pal.text_muted),
                    );
                    ui.label(
                        RichText::new(title)
                            .font(theme::font(14.5, Weight::SemiBold))
                            .color(pal.text),
                    );
                });
                ui.add_space(12.0);
            }
            add_contents(ui)
        })
}

pub fn section_label(ui: &mut Ui, pal: &Palette, label: &str) {
    ui.label(
        RichText::new(label)
            .font(theme::font(12.0, Weight::Medium))
            .color(pal.text_muted),
    );
}

pub fn divider(ui: &mut Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, pal.border);
}

/// Small status pill with a coloured dot.
pub fn status_pill(ui: &mut Ui, pal: &Palette, label: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme::font(12.0, Weight::Medium),
        color,
    );
    let width = 22.0 + galley.size().x + 12.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(13),
        theme::blend(color, pal.panel, 0.12),
        Stroke::new(1.0, theme::blend(color, pal.panel, 0.30)),
        StrokeKind::Inside,
    );
    ui.painter()
        .circle_filled(pos2(rect.left() + 12.0, rect.center().y), 3.5, color);
    ui.painter().galley(
        pos2(rect.left() + 21.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    response
}

/// A neutral outlined pill, e.g. "Teclado".
pub fn badge(ui: &mut Ui, pal: &Palette, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        theme::font(11.5, Weight::Medium),
        pal.text_muted,
    );
    let width = galley.size().x + 16.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 21.0), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(11),
        pal.muted,
        Stroke::new(1.0, pal.border),
        StrokeKind::Inside,
    );
    ui.painter().galley(
        pos2(rect.center().x - galley.size().x * 0.5, rect.center().y - galley.size().y * 0.5),
        galley,
        pal.text_muted,
    );
    response
}

/// Label on the left, value on the right.
pub fn kv_row(ui: &mut Ui, pal: &Palette, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(label)
                .font(theme::font(13.0, Weight::Regular))
                .color(pal.text_muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .font(theme::font(13.0, Weight::Medium))
                    .color(pal.text),
            );
        });
    });
}

/// Tinted inline notice with an icon and an optional action label.
/// Returns true when the action was clicked.
pub fn callout(
    ui: &mut Ui,
    pal: &Palette,
    color: Color32,
    icon: &str,
    text: &str,
    action: Option<&str>,
) -> bool {
    let mut clicked = false;
    Frame::new()
        .fill(theme::blend(color, pal.panel, 0.10))
        .stroke(Stroke::new(1.0, theme::blend(color, pal.panel, 0.28)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon)
                        .font(FontId::proportional(16.0))
                        .color(color),
                );
                ui.label(
                    RichText::new(text)
                        .font(theme::font(12.5, Weight::Regular))
                        .color(pal.text),
                );
                if let Some(action) = action {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if button(ui, pal, action, None, ButtonKind::Secondary).clicked() {
                            clicked = true;
                        }
                    });
                }
            });
        });
    clicked
}

/// A horizontal rule that doubles as a 1 px separator between list rows.
pub fn hairline(ui: &mut Ui, pal: &Palette, inset: f32) {
    let width = (ui.available_width() - inset * 2.0).max(0.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_min_size(pos2(rect.left() + inset, rect.top()), vec2(width, 1.0)),
        0.0,
        pal.border,
    );
}

/// Painter-friendly text helper for fixed rows.
pub fn paint_text(
    painter: &egui::Painter,
    pos: Pos2,
    text: &str,
    size: f32,
    weight: Weight,
    color: Color32,
) -> Rect {
    painter.text(pos, Align2::LEFT_TOP, text, theme::font(size, weight), color)
}
