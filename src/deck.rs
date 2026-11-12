use crate::art::Art;

pub struct DeckParams<'a> {
    pub art: Option<&'a Art>,
    pub disc_angle: f32,
    pub spin: bool,
    pub arm: f32,
    pub needle: f32,
    pub power: bool,
}

/// Full-window record player: procedural walnut plinth, metal platter, vinyl
/// with rotating video-art label, auto-drop tonearm, power LED.
pub fn draw(ui: &mut egui::Ui, rect: egui::Rect, p: &DeckParams) {
    let painter = ui.painter_at(rect);
    if !p.spin {
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 16, 13));
        draw_cover(ui, &painter, rect, p.art);
        return;
    }
    draw_wood(&painter, rect);

    // Platter sits left-of-center to leave room for the tonearm.
    let c = rect.center() + egui::vec2(-rect.width() * 0.07, 0.0);
    let r = (rect.width().min(rect.height()) / 2.0 - 24.0).max(40.0);

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

    draw_tonearm(&painter, c, r, p.arm, p.needle);

    // Power LED, bottom-left of the deck.
    let led = rect.left_bottom() + egui::vec2(24.0, -24.0);
    let col = if p.power {
        egui::Color32::from_rgb(230, 60, 40)
    } else {
        egui::Color32::from_rgb(70, 25, 20)
    };
    painter.circle_filled(led, 5.0, col);
}

/// Tonearm with pivot top-right of the platter. `arm` 0 = rest clip,
/// 1 = needle dropped; mid-swing the headshell lifts slightly.
fn draw_tonearm(painter: &egui::Painter, c: egui::Pos2, r: f32, arm: f32, needle: f32) {
    let pivot = c + egui::vec2(r * 1.04, -r * 1.04);
    let rest = c + egui::vec2(r * 1.30, r * 0.55);
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
