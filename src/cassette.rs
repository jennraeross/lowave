use crate::art::Art;
use crate::deck;

pub struct CassetteParams<'a> {
    pub art: Option<&'a Art>,
    pub play_time: f32,
    pub playing: bool,
    pub power: bool,
    pub volume: f32,
}

/// Walkman-style cassette player (unbranded): slim charcoal body with
/// red/yellow pinstripes, "STEREO CASSETTE PLAYER" face text, a big window
/// with two reels whose packs wind/unwind over a loop, J-card art label,
/// capstan + head, mechanical counter, and chunky piano-key transport on the
/// top edge — PLAY (green), STOP (red), REW/FF (grey, previous/next stream).
/// Reuses the wood plinth and shared bottom hardware (LED, play, fader).
pub fn draw(ui: &mut egui::Ui, rect: egui::Rect, p: &CassetteParams) -> deck::DeckAction {
    let mut action = deck::DeckAction::default();
    let painter = ui.painter_at(rect);

    crate::deck::draw_wood(&painter, rect);

    // Hardware geometry first so the player clears it.
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

    // Player body: landscape slab, centered, shrunk to clear hardware.
    let mut w = (rect.width() - 56.0).max(160.0);
    let mut h = w / 1.55;
    let max_h = rect.height() - 96.0;
    if h > max_h {
        h = max_h.max(110.0);
        w = h * 1.55;
    }
    let mut body = egui::Rect::from_center_size(rect.center(), egui::vec2(w, h));
    while body.width() > 160.0 && keep_out.iter().any(|k| k.intersects(body)) {
        let smaller = egui::vec2(body.width() * 0.97, body.height() * 0.97);
        body = egui::Rect::from_center_size(rect.center(), smaller);
    }

    // Charcoal shell with a red pinstripe waist.
    painter.rect_filled(body, 14.0, egui::Color32::from_rgb(24, 24, 28));
    painter.rect_stroke(
        body,
        14.0,
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(105, 100, 92)),
        egui::StrokeKind::Inside,
    );
    let stripe = egui::Rect::from_min_max(
        egui::pos2(body.min.x + 12.0, body.max.y - 30.0),
        egui::pos2(body.max.x - 12.0, body.max.y - 26.0),
    );
    painter.rect_filled(stripe, 2.0, egui::Color32::from_rgb(200, 60, 40));

    // Face text: yellow STEREO, white CASSETTE PLAYER, orange model line.
    painter.text(
        egui::pos2(body.min.x + 16.0, body.min.y + 10.0),
        egui::Align2::LEFT_TOP,
        "STEREO",
        egui::FontId::proportional(13.0),
        egui::Color32::from_rgb(230, 190, 60),
    );
    painter.text(
        egui::pos2(body.min.x + 16.0, body.min.y + 26.0),
        egui::Align2::LEFT_TOP,
        "CASSETTE PLAYER",
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(200, 198, 192),
    );
    painter.text(
        egui::pos2(body.min.x + 16.0, body.max.y - 22.0),
        egui::Align2::LEFT_TOP,
        "LOWAVE · AUTO REVERSE",
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(220, 120, 50),
    );

    // Big window with the cassette inside.
    let win = egui::Rect::from_min_max(
        egui::pos2(body.min.x + 14.0, body.min.y + 46.0),
        egui::pos2(body.max.x - 14.0, body.max.y - 38.0),
    );
    painter.rect_filled(win, 8.0, egui::Color32::from_rgb(10, 10, 12));
    painter.rect_stroke(
        win,
        8.0,
        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(120, 116, 108)),
        egui::StrokeKind::Inside,
    );
    // Inner cassette shell.
    let shell = win.shrink(6.0);
    painter.rect_filled(shell, 4.0, egui::Color32::from_rgb(38, 36, 34));

    // J-card label stripe with the video art, upright.
    let label_h = shell.height() * 0.30;
    let label = egui::Rect::from_min_max(
        egui::pos2(shell.min.x + 6.0, shell.min.y + 5.0),
        egui::pos2(shell.max.x - 6.0, shell.min.y + 5.0 + label_h),
    );
    if let Some(a) = p.art {
        let sized = egui::load::SizedTexture::new(a.square.id(), label.size());
        egui::Image::new(sized)
            .corner_radius(3.0)
            .paint_at(ui, label);
    } else {
        painter.rect_filled(label, 3.0, egui::Color32::from_rgb(140, 120, 90));
        painter.text(
            label.center(),
            egui::Align2::CENTER_CENTER,
            "LOWAVE · SIDE A",
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgb(40, 30, 20),
        );
    }

    // Reel area below the label.
    let reel_zone = egui::Rect::from_min_max(
        egui::pos2(shell.min.x + 6.0, label.max.y + 6.0),
        egui::pos2(shell.max.x - 6.0, shell.max.y - 16.0),
    );
    draw_reels(&painter, reel_zone, p);

    // Piano keys on the top edge: REW | PLAY | STOP | FF.
    let key_w = 56.0;
    let key_h = 26.0;
    let gap = 8.0;
    let total = key_w * 4.0 + gap * 3.0;
    let mut x = body.center().x - total / 2.0;
    let key_y = body.min.y - key_h * 0.45;
    let keys: [(&str, egui::Color32, egui::Color32); 4] = [
        ("«", egui::Color32::from_rgb(150, 148, 143), egui::Color32::from_rgb(30, 28, 26)),
        (
            if p.playing { "❚❚" } else { "▶" },
            egui::Color32::from_rgb(70, 160, 90),
            egui::Color32::from_rgb(235, 240, 235),
        ),
        ("■", egui::Color32::from_rgb(190, 70, 55), egui::Color32::from_rgb(240, 235, 230)),
        ("»", egui::Color32::from_rgb(150, 148, 143), egui::Color32::from_rgb(30, 28, 26)),
    ];
    for (i, (glyph, face_col, text_col)) in keys.iter().enumerate() {
        let key = egui::Rect::from_min_size(egui::pos2(x, key_y), egui::vec2(key_w, key_h));
        let resp = ui.allocate_rect(key, egui::Sense::click());
        let pressed = resp.is_pointer_button_down_on();
        let y_off = if pressed { 3.0 } else { 0.0 };
        let key = key.translate(egui::vec2(0.0, y_off));
        painter.rect_filled(key, 5.0, *face_col);
        painter.rect_stroke(
            key,
            5.0,
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(20, 18, 16)),
            egui::StrokeKind::Inside,
        );
        // Glyph: text except pause, which is hand-drawn (missing font glyph).
        if p.playing && i == 1 {
            let bw = 5.0;
            let bh = 13.0;
            let gap = 3.5;
            let cy = key.center().y - bh / 2.0;
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(key.center().x - gap - bw, cy),
                    egui::vec2(bw, bh),
                ),
                1.0,
                egui::Color32::from_rgb(235, 240, 235),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(key.center().x + gap, cy), egui::vec2(bw, bh)),
                1.0,
                egui::Color32::from_rgb(235, 240, 235),
            );
        } else {
            painter.text(
                key.center(),
                egui::Align2::CENTER_CENTER,
                *glyph,
                egui::FontId::proportional(15.0),
                *text_col,
            );
        }
        if resp.clicked() {
            match i {
                0 => action.prev = true,
                1 => action.toggle_play = true,
                2 => action.stop = true,
                _ => action.next = true,
            }
        }
        x += key_w + gap;
    }

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

fn draw_reels(painter: &egui::Painter, zone: egui::Rect, p: &CassetteParams) {
    // Packs wind over the loop: supply empties left→right.
    let frac = (p.play_time % LOOP_SECONDS) / LOOP_SECONDS;
    let hub_r = zone.height() * 0.16;
    let pack_full = (zone.height() * 0.44).min(zone.width() * 0.20);
    let pack_min = hub_r + 3.0;
    let supply_pack = pack_full + (pack_min - pack_full) * frac;
    let takeup_pack = pack_min + (pack_full - pack_min) * frac;
    let dx = zone.width() * 0.27;
    let left = egui::pos2(zone.center().x - dx, zone.center().y);
    let right = egui::pos2(zone.center().x + dx, zone.center().y);
    // Supply spins faster as it empties; take-up the reverse.
    let supply_ang = -p.play_time * 2.4 / (0.4 + frac);
    let takeup_ang = p.play_time * 2.4 / (1.4 - frac);

    draw_reel(painter, left, hub_r, supply_pack, supply_ang, p.playing);
    draw_reel(painter, right, hub_r, takeup_pack, takeup_ang, p.playing);

    // Tape path across the top and bottom of the packs.
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

    // Head gap + capstan nubs.
    let below = zone.max.y + 3.0;
    let head_cx = zone.center().x;
    for x in [head_cx - dx * 0.5, head_cx + dx * 0.5] {
        painter.circle_filled(egui::pos2(x, below + 5.0), 3.0, egui::Color32::from_rgb(160, 158, 152));
    }
    painter.rect_filled(
        egui::Rect::from_center_size(egui::pos2(head_cx, below + 5.0), egui::vec2(22.0, 10.0)),
        2.0,
        egui::Color32::from_rgb(90, 88, 84),
    );

    // Mechanical counter, bottom-right of the zone.
    let digits = format!("{:04}", (p.play_time as u64) % 10000);
    let counter = egui::Rect::from_min_max(
        egui::pos2(zone.max.x - 62.0, zone.max.y - 16.0),
        egui::pos2(zone.max.x - 2.0, zone.max.y),
    );
    painter.rect_filled(counter, 3.0, egui::Color32::from_rgb(8, 8, 10));
    painter.text(
        counter.center(),
        egui::Align2::CENTER_CENTER,
        digits,
        egui::FontId::monospace(12.0),
        egui::Color32::from_rgb(220, 215, 200),
    );
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
