use crate::art::Art;
use crate::deck;

pub struct CassetteParams<'a> {
    pub art: Option<&'a Art>,
    pub play_time: f32,
    pub playing: bool,
    pub power: bool,
    pub volume: f32,
}

/// Cassette deck alternate view: shell with a J-card art label, window with
/// two reels whose packs wind/unwind over a loop, tape path, capstan + head,
/// corner screws, and a mechanical counter. Reuses the wood plinth and the
/// shared chunky hardware (LED, play button, volume fader) from the vinyl
/// deck so both views feel like one machine.
pub fn draw(ui: &mut egui::Ui, rect: egui::Rect, p: &CassetteParams) -> deck::DeckAction {
    let mut action = deck::DeckAction::default();
    let painter = ui.painter_at(rect);

    crate::deck::draw_wood(&painter, rect);

    // Hardware geometry first so the cassette clears it.
    let base_y = rect.max.y - 40.0;
    let play_center = egui::pos2(rect.min.x + 96.0, base_y);
    let play_rect = egui::Rect::from_center_size(play_center, egui::vec2(96.0, 50.0));
    let fader_rect = egui::Rect::from_min_max(
        egui::pos2(
            (rect.max.x - 206.0).max(play_rect.max.x + 12.0),
            base_y - 26.0,
        ),
        egui::pos2(rect.max.x - 16.0, base_y + 26.0),
    );
    let led_rect = egui::Rect::from_center_size(
        egui::pos2(rect.min.x + 26.0, base_y),
        egui::vec2(12.0, 12.0),
    );
    let keep_out = [play_rect, fader_rect, led_rect];

    draw_cassette(ui, &painter, rect, p, &keep_out);

    // Shared hardware: LED, play, volume.
    let led = egui::pos2(rect.min.x + 26.0, base_y);
    painter.circle_filled(led, 5.0, egui::Color32::from_rgb(60, 55, 50));
    let led_col = if p.power {
        egui::Color32::from_rgb(230, 60, 40)
    } else {
        egui::Color32::from_rgb(70, 25, 20)
    };
    painter.circle_filled(led, 3.0, led_col);

    if deck::chunky_button(ui, &painter, play_center, egui::vec2(96.0, 50.0), p.power) {
        action.toggle_play = true;
    }
    if let Some(v) = deck::chunky_fader(ui, &painter, fader_rect, p.volume) {
        action.volume = Some(v);
    }

    action
}

/// Tape loop: a full "side" winds over this many seconds of play.
const LOOP_SECONDS: f32 = 2700.0;

fn draw_cassette(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    p: &CassetteParams,
    keep_out: &[egui::Rect],
) {
    // Cassette body: classic ~1.586:1 ratio, centered, shrunk to clear hardware.
    let mut w = (rect.width() - 48.0).max(120.0);
    let mut h = w / 1.586;
    let max_h = rect.height() - 48.0;
    if h > max_h {
        h = max_h.max(80.0);
        w = h * 1.586;
    }
    let mut body = egui::Rect::from_center_size(rect.center(), egui::vec2(w, h));
    while body.width() > 120.0
        && keep_out.iter().any(|k| k.intersects(body))
    {
        let smaller = egui::vec2(body.width() * 0.97, body.height() * 0.97);
        body = egui::Rect::from_center_size(rect.center(), smaller);
    }

    // Shell: dark body, lighter face plate.
    painter.rect_filled(body, 8.0, egui::Color32::from_rgb(28, 26, 24));
    painter.rect_stroke(
        body,
        8.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(95, 88, 78)),
        egui::StrokeKind::Inside,
    );
    let face = body.shrink(8.0);
    painter.rect_filled(face, 5.0, egui::Color32::from_rgb(52, 48, 44));

    // J-card label stripe with the video art, upright.
    let label_h = face.height() * 0.30;
    let label = egui::Rect::from_min_max(
        egui::pos2(face.min.x + 8.0, face.min.y + 6.0),
        egui::pos2(face.max.x - 8.0, face.min.y + 6.0 + label_h),
    );
    if let Some(a) = p.art {
        let sized = egui::load::SizedTexture::new(a.square.id(), label.size());
        egui::Image::new(sized)
            .corner_radius(4.0)
            .paint_at(ui, label);
    } else {
        painter.rect_filled(label, 4.0, egui::Color32::from_rgb(140, 120, 90));
        painter.text(
            label.center(),
            egui::Align2::CENTER_CENTER,
            "LOWAVE · SIDE A",
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgb(40, 30, 20),
        );
    }

    // Window: smoked glass over the reels.
    let win = egui::Rect::from_min_max(
        egui::pos2(face.min.x + 10.0, label.max.y + 8.0),
        egui::pos2(face.max.x - 10.0, face.max.y - 34.0),
    );
    painter.rect_filled(win, 6.0, egui::Color32::from_rgb(12, 11, 10));
    painter.rect_stroke(
        win,
        6.0,
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(110, 105, 95)),
        egui::StrokeKind::Inside,
    );

    // Reel geometry. Packs wind over the loop: supply empties left→right.
    let frac = (p.play_time % LOOP_SECONDS) / LOOP_SECONDS;
    let hub_r = win.height() * 0.13;
    let pack_full = win.height() * 0.40;
    let pack_min = hub_r + 3.0;
    let supply_pack = pack_full + (pack_min - pack_full) * frac;
    let takeup_pack = pack_min + (pack_full - pack_min) * frac;
    let dx = win.width() * 0.26;
    let left = egui::pos2(win.center().x - dx, win.center().y);
    let right = egui::pos2(win.center().x + dx, win.center().y);
    // Supply spins faster as it empties; take-up the reverse.
    let supply_ang = -p.play_time * 2.4 / (0.4 + frac);
    let takeup_ang = p.play_time * 2.4 / (1.4 - frac);

    draw_reel(painter, left, hub_r, supply_pack, supply_ang, p.playing);
    draw_reel(painter, right, hub_r, takeup_pack, takeup_ang, p.playing);

    // Tape path: tangent-ish lines across the top and bottom of the packs.
    let tape_col = egui::Color32::from_rgb(74, 52, 34);
    painter.line_segment(
        [
            egui::pos2(left.x, left.y - supply_pack),
            egui::pos2(right.x, right.y - takeup_pack),
        ],
        egui::Stroke::new(2.0_f32, tape_col),
    );
    painter.line_segment(
        [
            egui::pos2(left.x, left.y + supply_pack),
            egui::pos2(right.x, right.y + takeup_pack),
        ],
        egui::Stroke::new(2.0_f32, tape_col),
    );

    // Head gap + capstan nubs below the window.
    let below = win.max.y + 4.0;
    let head_cx = win.center().x;
    for (i, x) in [head_cx - dx * 0.5, head_cx + dx * 0.5].iter().enumerate() {
        let nub = egui::pos2(*x, below + 8.0);
        painter.circle_filled(nub, 4.0, egui::Color32::from_rgb(160, 158, 152));
        let _ = i;
    }
    painter.rect_filled(
        egui::Rect::from_center_size(
            egui::pos2(head_cx, below + 8.0),
            egui::vec2(26.0, 12.0),
        ),
        2.0,
        egui::Color32::from_rgb(90, 88, 84),
    );

    // Mechanical counter, bottom-right of the face.
    let digits = format!("{:04}", (p.play_time as u64) % 10000);
    let counter = egui::Rect::from_min_max(
        egui::pos2(face.max.x - 76.0, face.max.y - 26.0),
        egui::pos2(face.max.x - 8.0, face.max.y - 8.0),
    );
    painter.rect_filled(counter, 3.0, egui::Color32::from_rgb(8, 8, 10));
    painter.text(
        counter.center(),
        egui::Align2::CENTER_CENTER,
        digits,
        egui::FontId::monospace(14.0),
        egui::Color32::from_rgb(220, 215, 200),
    );

    // Corner screws.
    for corner in [
        body.left_top(),
        body.right_top(),
        body.left_bottom(),
        body.right_bottom(),
    ] {
        let s = corner
            + egui::vec2(
                if corner.x < body.center().x { 9.0 } else { -9.0 },
                if corner.y < body.center().y { 9.0 } else { -9.0 },
            );
        painter.circle_filled(s, 4.0, egui::Color32::from_rgb(120, 118, 112));
        painter.line_segment(
            [s + egui::vec2(-2.5, 1.5), s + egui::vec2(2.5, -1.5)],
            egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(50, 48, 44)),
        );
    }
}

/// Reel: brown tape pack, white hub with spokes rotating at `angle`.
fn draw_reel(
    painter: &egui::Painter,
    c: egui::Pos2,
    hub_r: f32,
    pack_r: f32,
    angle: f32,
    spinning: bool,
) {
    // Tape pack.
    painter.circle_filled(c, pack_r, egui::Color32::from_rgb(62, 44, 30));
    painter.circle_stroke(
        c,
        pack_r,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(110, 85, 60)),
    );
    // Pack sheen.
    painter.circle_stroke(
        c,
        pack_r * 0.7,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(150, 120, 90, 40)),
    );
    // Hub.
    painter.circle_filled(c, hub_r, egui::Color32::from_rgb(215, 212, 205));
    painter.circle_filled(c, hub_r * 0.45, egui::Color32::from_rgb(30, 28, 26));
    // Spokes: 3 arms; static when paused.
    let a = if spinning { angle } else { 0.6 };
    for i in 0..3 {
        let t = a + i as f32 * std::f32::consts::TAU / 3.0;
        let dir = egui::vec2(t.cos(), t.sin());
        painter.line_segment(
            [c + dir * hub_r * 0.45, c + dir * (hub_r - 1.5)],
            egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(60, 58, 54)),
        );
    }
}
