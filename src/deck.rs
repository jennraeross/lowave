use crate::art::Art;

pub struct DeckParams<'a> {
    pub art: Option<&'a Art>,
    pub disc_angle: f32,
    pub spin: bool,
    pub arm: f32,
    pub needle: f32,
    pub power: bool,
    pub volume: f32,
}

/// Interactions from the deck's hardware controls.
#[derive(Default)]
pub struct DeckAction {
    pub toggle_play: bool,
    pub volume: Option<f32>,
}

/// Full-window record player: procedural walnut plinth, centered metal
/// platter with vinyl + rotating video-art label, auto-drop tonearm.
/// Hardware (LED, play button, volume fader) docks in the bottom corners so
/// it never overlaps the record; the album stays centered horizontally and
/// vertically.
pub fn draw(ui: &mut egui::Ui, rect: egui::Rect, p: &DeckParams) -> DeckAction {
    let mut action = DeckAction::default();
    let painter = ui.painter_at(rect);

    draw_wood(&painter, rect);

    if p.spin {
        draw_platter(ui, &painter, rect, p);
    } else {
        draw_cover(ui, &painter, rect, p.art);
    }

    // Hardware docks in the bottom corners, clear of the centered record.
    let base_y = rect.max.y - 40.0;
    let led = egui::pos2(rect.min.x + 26.0, base_y);
    painter.circle_filled(led, 5.0, egui::Color32::from_rgb(60, 55, 50));
    let led_col = if p.power {
        egui::Color32::from_rgb(230, 60, 40)
    } else {
        egui::Color32::from_rgb(70, 25, 20)
    };
    painter.circle_filled(led, 3.0, led_col);

    let play_center = egui::pos2(rect.min.x + 96.0, base_y);
    if chunky_button(ui, &painter, play_center, egui::vec2(96.0, 50.0), p.power) {
        action.toggle_play = true;
    }

    let fader_rect = egui::Rect::from_min_max(
        egui::pos2(
            (rect.max.x - 206.0).max(play_center.x + 60.0),
            base_y - 26.0,
        ),
        egui::pos2(rect.max.x - 16.0, base_y + 26.0),
    );
    if let Some(v) = chunky_fader(ui, &painter, fader_rect, p.volume) {
        action.volume = Some(v);
    }

    action
}

/// Big metal push-button. Returns true when clicked.
fn chunky_button(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    center: egui::Pos2,
    size: egui::Vec2,
    active: bool,
) -> bool {
    let rect = egui::Rect::from_center_size(center, size);
    let resp = ui.allocate_rect(rect, egui::Sense::click());
    let pressed = resp.is_pointer_button_down_on();
    let hover = resp.hovered();

    // Bezel.
    painter.rect_filled(rect, 10.0, egui::Color32::from_rgb(22, 20, 18));
    painter.rect_stroke(
        rect,
        10.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(90, 82, 72)),
        egui::StrokeKind::Inside,
    );
    // Face: pressed face sits 2px lower.
    let inset = if pressed { 7.0 } else { 5.0 };
    let face = rect.shrink(inset);
    let mut face_col = egui::Color32::from_rgb(168, 164, 158);
    if hover {
        face_col = egui::Color32::from_rgb(186, 182, 176);
    }
    if active {
        face_col = egui::Color32::from_rgb(190, 170, 150);
    }
    painter.rect_filled(face, 7.0, face_col);
    // Top highlight.
    painter.line_segment(
        [
            egui::pos2(face.min.x + 8.0, face.min.y + 2.0),
            egui::pos2(face.max.x - 8.0, face.min.y + 2.0),
        ],
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 90)),
    );
    // Glyph: text triangle for play, hand-drawn bars for pause
    // (the ❚❚ glyph is missing from the bundled font).
    if active {
        let bw = 7.0;
        let bh = 22.0;
        let gap = 5.0;
        let cy = face.center().y - bh / 2.0;
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(face.center().x - gap - bw, cy),
                egui::vec2(bw, bh),
            ),
            1.5,
            egui::Color32::from_rgb(30, 26, 22),
        );
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(face.center().x + gap, cy), egui::vec2(bw, bh)),
            1.5,
            egui::Color32::from_rgb(30, 26, 22),
        );
    } else {
        painter.text(
            face.center(),
            egui::Align2::CENTER_CENTER,
            "▶",
            egui::FontId::proportional(24.0),
            egui::Color32::from_rgb(30, 26, 22),
        );
    }
    resp.clicked()
}

/// Horizontal fader: groove track + knurled metal knob. Drag or click the
/// track to set. Returns the new value when changed.
fn chunky_fader(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    value: f32,
) -> Option<f32> {
    let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    painter.text(
        egui::pos2(rect.min.x, rect.min.y + 2.0),
        egui::Align2::LEFT_TOP,
        "VOLUME",
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(150, 135, 120),
    );
    let track = egui::Rect::from_min_max(
        egui::pos2(rect.min.x, rect.center().y + 2.0),
        egui::pos2(rect.max.x, rect.center().y + 10.0),
    );
    painter.rect_filled(track, 4.0, egui::Color32::from_rgb(12, 10, 8));
    painter.rect_stroke(
        track,
        4.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(90, 82, 72)),
        egui::StrokeKind::Inside,
    );
    // Fill portion.
    let fill_w = (track.width() * value.clamp(0.0, 1.0)).max(4.0);
    painter.rect_filled(
        egui::Rect::from_min_max(track.min, egui::pos2(track.min.x + fill_w, track.max.y)),
        4.0,
        egui::Color32::from_rgb(150, 110, 60),
    );
    // Knob.
    let kx = track.min.x + track.width() * value.clamp(0.0, 1.0);
    let knob = egui::Rect::from_center_size(
        egui::pos2(kx, track.center().y),
        egui::vec2(22.0, 34.0),
    );
    painter.rect_filled(knob, 5.0, egui::Color32::from_rgb(175, 170, 163));
    painter.rect_stroke(
        knob,
        5.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(60, 55, 50)),
        egui::StrokeKind::Inside,
    );
    for i in -1..=1 {
        let x = knob.center().x + i as f32 * 6.0;
        painter.line_segment(
            [
                egui::pos2(x, knob.min.y + 6.0),
                egui::pos2(x, knob.max.y - 6.0),
            ],
            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(90, 85, 78)),
        );
    }

    if resp.clicked() || resp.dragged() {
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos().or(i.pointer.press_origin())) {
            let v = ((pos.x - track.min.x) / track.width()).clamp(0.0, 1.0);
            if (v - value).abs() > 0.001 {
                return Some(v);
            }
        } else if resp.dragged() {
            // Keyboard-driven drag without a pointer position: nudge.
            let d = resp.drag_delta().x / track.width();
            if d != 0.0 {
                return Some((value + d).clamp(0.0, 1.0));
            }
        }
    }
    None
}

fn draw_platter(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    p: &DeckParams,
) {
    // Platter centered in the window, horizontally and vertically.
    let c = rect.center();
    let r = (rect.width().min(rect.height()) / 2.0 - 12.0).max(40.0);

    // Metal platter + rubber mat.
    painter.circle_filled(c, r + 10.0, egui::Color32::from_rgb(150, 148, 145));
    painter.circle_filled(c, r + 6.0, egui::Color32::from_rgb(52, 52, 56));
    painter.circle_filled(c, r + 2.0, egui::Color32::from_rgb(16, 16, 18));

    // Vinyl.
    painter.circle_filled(c, r, egui::Color32::from_rgb(10, 10, 12));
    painter.circle_stroke(
        c,
        r,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(60, 55, 50)),
    );

    // Rotating video-art label.
    let label_r = r * 0.62;
    if let Some(a) = p.art {
        let side = label_r * 2.0;
        let irect = egui::Rect::from_center_size(c, egui::vec2(side, side));
        let sized = egui::load::SizedTexture::new(a.disc.id(), irect.size());
        egui::Image::new(sized)
            .rotate(p.disc_angle, egui::Vec2::splat(0.5))
            .paint_at(ui, irect);
    } else {
        painter.circle_filled(c, label_r, egui::Color32::from_rgb(32, 26, 22));
        painter.text(
            c,
            egui::Align2::CENTER_CENTER,
            "LOWAVE",
            egui::FontId::proportional(18.0),
            egui::Color32::from_rgb(150, 130, 110),
        );
    }

    // Groove sheen on the exposed vinyl ring.
    for i in 1..=6 {
        let gr = label_r + (r - label_r) * (i as f32 / 7.0);
        painter.circle_stroke(
            c,
            gr,
            egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgba_unmultiplied(120, 110, 100, 28),
            ),
        );
    }
    // Spindle.
    painter.circle_filled(c, 5.0, egui::Color32::from_rgb(180, 170, 160));
    painter.circle_filled(c, 2.0, egui::Color32::from_rgb(16, 14, 12));

    draw_tonearm(painter, rect, c, r, p.arm, p.needle);
}

/// Tonearm with pivot top-right of the platter. `arm` 0 = rest clip,
/// 1 = needle dropped; mid-swing the headshell lifts slightly.
fn draw_tonearm(
    painter: &egui::Painter,
    bounds: egui::Rect,
    c: egui::Pos2,
    r: f32,
    arm: f32,
    needle: f32,
) {
    let mut pivot = c + egui::vec2(r * 1.04, -r * 1.04);
    // Keep the pivot on the deck.
    pivot.x = pivot.x.min(bounds.max.x - 24.0);
    pivot.y = pivot.y.max(bounds.min.y + 24.0);
    let mut rest = c + egui::vec2(r * 1.30, r * 0.55);
    rest.x = rest.x.min(bounds.max.x - 14.0);
    rest.y = rest.y.min(bounds.max.y - 14.0);
    // Groove point: right side, slightly below center, drifting inward.
    let g = r * (0.88 - 0.50 * needle.clamp(0.0, 1.0));
    let drop = c + egui::vec2(g * 0.91, g * 0.42);

    // Smoothstep the swing; lift the headshell mid-transit.
    let a = arm.clamp(0.0, 1.0);
    let eased = a * a * (3.0 - 2.0 * a);
    let lift = (std::f32::consts::PI * a).sin() * 12.0;
    let tip = egui::pos2(
        rest.x + (drop.x - rest.x) * eased,
        rest.y + (drop.y - rest.y) * eased - lift,
    );

    // Rest clip.
    painter.circle_stroke(
        rest,
        6.0,
        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(90, 85, 80)),
    );

    // Counterweight stub behind the pivot.
    let dir = if (tip - pivot).length() > 0.01 {
        (tip - pivot).normalized()
    } else {
        egui::vec2(1.0, 0.0)
    };
    let cw = pivot - dir * 36.0;
    painter.line_segment(
        [pivot, cw],
        egui::Stroke::new(7.0_f32, egui::Color32::from_rgb(45, 45, 50)),
    );
    painter.circle_filled(cw, 10.0, egui::Color32::from_rgb(70, 70, 78));

    // Arm tube.
    painter.line_segment(
        [pivot, tip],
        egui::Stroke::new(5.0_f32, egui::Color32::from_rgb(195, 195, 200)),
    );
    // Headshell: short angled segment at the tip.
    let tangent = egui::vec2(-dir.y, dir.x);
    painter.line_segment(
        [tip - tangent * 4.0, tip + tangent * 14.0],
        egui::Stroke::new(7.0_f32, egui::Color32::from_rgb(30, 30, 34)),
    );
    painter.circle_filled(tip, 2.5, egui::Color32::from_rgb(220, 220, 225));

    // Pivot base on top.
    painter.circle_filled(pivot, 15.0, egui::Color32::from_rgb(38, 38, 44));
    painter.circle_filled(pivot, 10.0, egui::Color32::from_rgb(72, 72, 80));
    painter.circle_filled(pivot, 4.0, egui::Color32::from_rgb(160, 160, 168));
}

/// Walnut plinth: vertical gradient bands + deterministic grain + plank
/// seams. The hash-based grain is stable frame to frame (no shimmer).
fn draw_wood(painter: &egui::Painter, rect: egui::Rect) {
    let dark = egui::Color32::from_rgb(46, 28, 16);
    let light = egui::Color32::from_rgb(96, 62, 34);
    painter.rect_filled(rect, 0.0, dark);
    let bands = 48;
    for i in 0..bands {
        let t0 = i as f32 / bands as f32;
        let t1 = (i + 1) as f32 / bands as f32;
        let col = lerp_color(dark, light, t0 * 0.9);
        let y0 = rect.min.y + rect.height() * t0;
        let y1 = rect.min.y + rect.height() * t1 + 1.0;
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x, y0),
                egui::pos2(rect.max.x, y1),
            ),
            0.0,
            col,
        );
    }
    // Grain lines.
    for i in 0..42 {
        let h = hash(i);
        let y = rect.min.y + h * rect.height();
        let slope = (hash(i + 999) - 0.5) * 8.0;
        painter.line_segment(
            [
                egui::pos2(rect.min.x, y),
                egui::pos2(rect.max.x, y + slope),
            ],
            egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgba_unmultiplied(22, 12, 6, 24 + (h * 40.0) as u8),
            ),
        );
    }
    // Plank seams.
    for k in 1..3 {
        let y = rect.min.y + rect.height() * (k as f32 / 3.0);
        painter.line_segment(
            [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
            egui::Stroke::new(2.0_f32, egui::Color32::from_rgba_unmultiplied(15, 8, 4, 160)),
        );
    }
    // Dark edge frame.
    painter.rect_stroke(
        rect.shrink(3.0),
        0.0,
        egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(24, 14, 8)),
        egui::StrokeKind::Inside,
    );
}

/// Upright square album cover used when spin is off.
fn draw_cover(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    art: Option<&Art>,
) {
    let c = rect.center();
    let side = (rect.width().min(rect.height()) - 48.0).max(80.0);
    let crect = egui::Rect::from_center_size(c, egui::vec2(side, side));
    painter.rect_filled(
        crect.translate(egui::vec2(0.0, 6.0)),
        10.0,
        egui::Color32::from_black_alpha(120),
    );
    if let Some(a) = art {
        let sized = egui::load::SizedTexture::new(a.square.id(), crect.size());
        egui::Image::new(sized)
            .corner_radius(10.0)
            .paint_at(ui, crect);
    } else {
        painter.rect_filled(crect, 10.0, egui::Color32::from_rgb(32, 26, 22));
        painter.text(
            c,
            egui::Align2::CENTER_CENTER,
            "LOWAVE",
            egui::FontId::proportional(20.0),
            egui::Color32::from_rgb(150, 130, 110),
        );
    }
}

fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

fn hash(i: u32) -> f32 {
    ((i as f32 * 12.9898).sin() * 43758.55).fract().abs()
}
