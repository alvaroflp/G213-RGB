//! The device illustration: a stylised full-size keyboard painted with the
//! current zone colours.
//!
//! The G213 only has five lighting areas, so the drawing groups its keys into
//! the same five areas and lets the lighting tab select one by clicking it.

use crate::theme::{self, Palette};
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};
use std::ops::Range;

const KEY_ROWS: [[f32; 15]; 5] = [
    [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.5, 0.5],
    [1.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.5, 0.0],
    [1.75, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.25, 0.0, 0.0],
    [2.25, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.75, 0.0, 0.0],
    [1.25, 1.25, 1.25, 6.25, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
];

const MAIN_COLS: f32 = 15.0;
const NAV_COLS: f32 = 3.0;
const NUMPAD_COLS: f32 = 4.0;
const GAP: f32 = 0.35;
const TOTAL_COLS: f32 = MAIN_COLS + GAP + NAV_COLS + GAP + NUMPAD_COLS;

pub struct KeyboardState {
    pub zone_colors: [Color32; 5],
    pub enabled: bool,
    pub selected_zone: Option<usize>,
}

/// Paints the keyboard at `width` and returns the click response plus the zone
/// that was clicked (if any).
pub fn keyboard(
    ui: &mut Ui,
    pal: &Palette,
    width: f32,
    state: &KeyboardState,
    interactive: bool,
) -> (Response, Option<usize>) {
    let width = width.max(220.0);
    let height = width * 0.315;
    let sense = if interactive {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);

    let unit = rect.width() / (TOTAL_COLS + 0.7);
    let key_gap = (unit * 0.10).clamp(1.3, 3.2);
    let key_radius = CornerRadius::same((unit * 0.10).clamp(1.5, 3.5) as u8);

    let keys_h = unit * 6.15;
    let body = Rect::from_center_size(
        rect.center(),
        vec2(rect.width() - unit * 0.2, keys_h + unit * 0.62),
    );

    let key_base = Color32::from_rgb(0x24, 0x24, 0x2b);
    let key_off = Color32::from_rgb(0x3a, 0x3a, 0x44);
    let body_fill = Color32::from_rgb(0x1b, 0x1b, 0x21);
    let body_border = Color32::from_rgb(0x3a, 0x3a, 0x45);

    // Soft ambient glow behind the keyboard in the active colour.
    if state.enabled {
        let color = match state.selected_zone {
            Some(zone) => state.zone_colors[zone],
            None => average_color(&state.zone_colors),
        };
        for (index, spread) in [26.0f32, 20.0, 15.0, 10.0, 6.0].into_iter().enumerate() {
            let alpha = [10u8, 14, 18, 24, 30][index];
            ui.painter().rect_filled(
                body.expand(spread),
                CornerRadius::same((14.0 + spread) as u8),
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
            );
        }
    }

    ui.painter().rect(
        body,
        CornerRadius::same(10),
        body_fill,
        Stroke::new(1.0, body_border),
        StrokeKind::Inside,
    );

    let keys_left = body.left() + unit * 0.35;
    let keys_top = body.top() + unit * 0.31;
    let main_width = MAIN_COLS * unit;
    let nav_left = keys_left + main_width + GAP * unit;
    let numpad_left = nav_left + NAV_COLS * unit + GAP * unit;

    // Function row: one strip above the main block, split into 16 keys.
    let function_h = unit * 0.55;
    let function_gap = unit * 0.12;
    let function_keys = 16;
    let function_w = (TOTAL_COLS * unit - (function_keys - 1) as f32 * key_gap) / function_keys as f32;
    for index in 0..function_keys {
        let x = keys_left + index as f32 * (function_w + key_gap);
        let key = Rect::from_min_size(
            pos2(x, keys_top),
            vec2(function_w, function_h),
        );
        let zone = zone_for_x(key.center().x, keys_left, main_width, nav_left, numpad_left, unit);
        paint_key(ui, key, key_radius, key_color(state, zone, key_base, key_off));
    }

    // Main block.
    let main_top = keys_top + function_h + function_gap;
    for (row_index, row) in KEY_ROWS.iter().enumerate() {
        let mut x = keys_left;
        let y = main_top + row_index as f32 * (unit + function_gap);
        for width_units in row {
            if *width_units <= 0.0 {
                continue;
            }
            let w = width_units * unit;
            let key = Rect::from_min_size(
                pos2(x, y),
                vec2(w - key_gap, unit - key_gap),
            );
            let zone = zone_for_x(key.center().x, keys_left, main_width, nav_left, numpad_left, unit);
            paint_key(ui, key, key_radius, key_color(state, zone, key_base, key_off));
            x += w;
        }
    }

    // Navigation cluster: 3 × 5.
    let nav_key_w = NAV_COLS * unit / 3.0;
    for row in 0..5 {
        for col in 0..3 {
            let key = Rect::from_min_size(
                pos2(
                    nav_left + col as f32 * nav_key_w,
                    main_top + row as f32 * (unit + function_gap),
                ),
                vec2(nav_key_w - key_gap, unit - key_gap),
            );
            paint_key(ui, key, key_radius, key_color(state, 3, key_base, key_off));
        }
    }

    // Numpad: 4 × 5.
    let num_key_w = NUMPAD_COLS * unit / 4.0;
    for row in 0..5 {
        for col in 0..4 {
            let key = Rect::from_min_size(
                pos2(
                    numpad_left + col as f32 * num_key_w,
                    main_top + row as f32 * (unit + function_gap),
                ),
                vec2(num_key_w - key_gap, unit - key_gap),
            );
            paint_key(ui, key, key_radius, key_color(state, 4, key_base, key_off));
        }
    }

    // Branding, kept tiny so it reads as a texture rather than a label.
    ui.painter().text(
        pos2(bottom_left(body).x + unit * 0.4, body.bottom() - unit * 0.22),
        Align2::LEFT_BOTTOM,
        "G213",
        FontId::proportional((unit * 0.26).clamp(7.0, 10.0)),
        Color32::from_rgba_unmultiplied(255, 255, 255, 55),
    );

    // Selected zone outline.
    if let Some(zone) = state.selected_zone {
        let block = zone_block(zone, keys_left, keys_top, main_width, nav_left, numpad_left, unit, function_h + function_gap);
        ui.painter().rect(
            block,
            CornerRadius::same(8),
            theme::blend(theme::ACCENT, body_fill, 0.08),
            Stroke::new(2.0, theme::ACCENT),
            StrokeKind::Inside,
        );
    }

    let clicked = if interactive {
        response
            .interact_pointer_pos()
            .filter(|pos| body.contains(*pos) && response.clicked())
            .map(|pos| zone_for_x(pos.x, keys_left, main_width, nav_left, numpad_left, unit))
    } else {
        None
    };

    let response = if interactive {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    };
    let _ = pal;
    (response, clicked)
}

fn bottom_left(rect: Rect) -> Pos2 {
    pos2(rect.left(), rect.bottom())
}

fn average_color(colors: &[Color32; 5]) -> Color32 {
    let mut sum = [0u32; 3];
    for color in colors {
        sum[0] += u32::from(color.r());
        sum[1] += u32::from(color.g());
        sum[2] += u32::from(color.b());
    }
    let n = colors.len() as u32;
    Color32::from_rgb(
        (sum[0] / n) as u8,
        (sum[1] / n) as u8,
        (sum[2] / n) as u8,
    )
}

fn zone_for_x(
    x: f32,
    main_left: f32,
    main_width: f32,
    nav_left: f32,
    numpad_left: f32,
    unit: f32,
) -> usize {
    if x < main_left {
        return 0;
    }
    if x < main_left + main_width {
        let third = main_width / 3.0;
        if x < main_left + third {
            0
        } else if x < main_left + third * 2.0 {
            1
        } else {
            2
        }
    } else if x < numpad_left - unit * GAP * 0.5 {
        // `nav_left` is the left edge of the arrow/home cluster.
        let _ = nav_left;
        3
    } else {
        4
    }
}

#[allow(clippy::too_many_arguments)]
fn zone_block(
    zone: usize,
    main_left: f32,
    top: f32,
    main_width: f32,
    nav_left: f32,
    numpad_left: f32,
    unit: f32,
    function_space: f32,
) -> Rect {
    let bottom = top + function_space + unit * 5.0 + unit * 0.4;
    let left = main_left;
    let third = main_width / 3.0;
    let (x, w) = match zone {
        0 => (main_left, third),
        1 => (main_left + third, third),
        2 => (main_left + third * 2.0, third),
        3 => (nav_left, unit * NAV_COLS),
        _ => (numpad_left, unit * NUMPAD_COLS),
    };
    let _ = left;
    Rect::from_min_max(pos2(x, top - unit * 0.15), pos2(x + w, bottom))
}

fn key_color(
    state: &KeyboardState,
    zone: usize,
    key_base: Color32,
    key_off: Color32,
) -> Color32 {
    if !state.enabled {
        return key_off;
    }
    let color = state.zone_colors[zone];
    // Keep a hint of the keycap base so keys read as separate objects.
    theme::blend(color, key_base, 0.82)
}

fn paint_key(ui: &Ui, rect: Rect, radius: CornerRadius, fill: Color32) {
    ui.painter()
        .rect_filled(rect, radius, fill);
    // A subtle top highlight gives the flat keys a cap-like edge.
    let highlight = Rect::from_min_size(rect.min, vec2(rect.width(), 1.0));
    ui.painter().rect_filled(
        highlight,
        CornerRadius {
            nw: radius.nw,
            ne: radius.ne,
            sw: 0,
            se: 0,
        },
        Color32::from_rgba_unmultiplied(255, 255, 255, 18),
    );
}

/// Ranges of the five zones, used by the settings list to draw a tiny legend.
pub fn zone_ranges() -> [Range<usize>; 5] {
    [0..1, 1..2, 2..3, 3..4, 4..5]
}
