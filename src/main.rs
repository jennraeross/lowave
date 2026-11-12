mod art;
mod config;
mod deck;
mod playback;
mod ytdlp;

use art::ArtCache;
use config::{AppConfig, UserStream};
use eframe::egui;
use playback::{PlaybackController, PlaybackStatus};
use ytdlp::LiveEntry;

fn main() -> eframe::Result<()> {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "lowave",
        options,
        Box::new(|_cc| Ok(Box::new(LowaveApp::new()))),
    )
}

struct LowaveApp {
    cfg: AppConfig,
    entries: Vec<LiveEntry>,
    selected: Option<usize>,
    status_text: String,
    ytdlp_version: String,
    player: PlaybackController,
    art: ArtCache,
    disc_angle: f32,
    arm: f32,
    needle: f32,
    new_name: String,
    new_url: String,
    loading: bool,
    show_sidebar: bool,
    last_frame: std::time::Instant,
    last_pointer: Option<egui::Pos2>,
    last_move: std::time::Instant,
}

impl LowaveApp {
    fn new() -> Self {
        let cfg = config::load();
        let volume = cfg.volume;
        let ytdlp_version =
            ytdlp::check_ytdlp().unwrap_or_else(|e| format!("missing ({e:#})"));
        let now = std::time::Instant::now();
        let mut app = Self {
            cfg,
            entries: Vec::new(),
            selected: None,
            status_text: "idle".to_string(),
            ytdlp_version,
            player: PlaybackController::new(volume),
            art: ArtCache::new(),
            disc_angle: 0.0,
            arm: 0.0,
            needle: 0.0,
            new_name: String::new(),
            new_url: String::new(),
            loading: true,
            show_sidebar: false,
            last_frame: now,
            last_pointer: None,
            last_move: now,
        };
        app.refresh_entries();
        app
    }

    fn refresh_entries(&mut self) {
        self.loading = true;
        // Resolve synchronously for V1 simplicity (fast enough for a few URLs).
        // Lofi live-tab expands to multiple entries; plain watch URLs stay single.
        let mut out = Vec::new();
        for s in &self.cfg.streams {
            for e in ytdlp::list_live(&s.url) {
                // Prefer user label when the resolve fell back to a single entry.
                let title = if e.url == s.url && e.title == s.url {
                    s.name.clone()
                } else {
                    e.title.clone()
                };
                out.push(LiveEntry { title, ..e });
            }
        }
        // Live entries first.
        out.sort_by_key(|e| !e.is_live);
        self.entries = out;
        // Re-select: last-played stream if still present, else the classic
        // beats to relax/study to, else the first live entry.
        if self.selected.is_none() && !self.entries.is_empty() {
            self.selected = self
                .entries
                .iter()
                .position(|e| Some(&e.url) == self.cfg.last_url.as_ref())
                .or_else(|| {
                    self.entries
                        .iter()
                        .position(|e| e.id == "rFZHOHl-L8A")
                })
                .or_else(|| self.entries.iter().position(|e| e.is_live))
                .or(Some(0));
        }
        self.loading = false;
    }

    /// Select entry `i` and start playing it immediately; remembers it.
    fn play_entry(&mut self, i: usize) {
        if let Some(e) = self.entries.get(i) {
            self.selected = Some(i);
            self.cfg.last_url = Some(e.url.clone());
            let _ = config::save(&self.cfg);
            self.needle = 0.0;
            self.player.play(e.url.clone());
        }
    }

    fn toggle_play(&mut self) {
        match self.player.status() {
            PlaybackStatus::Playing(_) => self.player.stop(),
            _ => {
                if let Some(i) = self.selected {
                    self.play_entry(i);
                }
            }
        }
    }
}

impl eframe::App for LowaveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        let st = self.player.status();
        let playing = matches!(st, PlaybackStatus::Playing(_));
        self.status_text = match &st {
            PlaybackStatus::Stopped => "stopped".to_string(),
            PlaybackStatus::Buffering(u) => format!("buffering… {u}"),
            PlaybackStatus::Playing(u) => format!("playing {u}"),
            PlaybackStatus::Error(e) => format!("error: {e}"),
        };
        // Spin the disc while playing; slow crawl while buffering.
        // Disabled entirely via the sidebar toggle.
        let buffering = matches!(st, PlaybackStatus::Buffering(_));
        if self.cfg.spin {
            if playing {
                self.disc_angle += dt * 0.2;
            } else if buffering {
                self.disc_angle += dt * 0.05;
            }
        }
        // Tonearm follows playback: drops when playing, lifts back on stop.
        let arm_target = if playing { 1.0 } else { 0.0 };
        let step = dt * 1.2;
        self.arm += (arm_target - self.arm).clamp(-step, step);
        if playing {
            self.needle = (self.needle + dt * 0.002).min(1.0);
        }

        // Track pointer movement for auto-hide.
        if ctx.pointer_hover_pos() != self.last_pointer {
            self.last_pointer = ctx.pointer_hover_pos();
            self.last_move = now;
        }
        let idle = now.duration_since(self.last_move).as_secs_f32();
        let controls_visible = self.show_sidebar || !playing || idle < 2.0;

        // Dark analog theme, no panel chrome.
        ctx.set_visuals(egui::Visuals {
            dark_mode: true,
            panel_fill: egui::Color32::from_rgb(16, 14, 12),
            ..egui::Visuals::dark()
        });

        // Full-window centerpiece: spinning vinyl disc, or — when spin is
        // off — the art as an upright square album cover.
        // Resolve art before borrowing ui mutably.
        let (sel_id, sel_thumb) = match self.selected.and_then(|i| self.entries.get(i)) {
            Some(e) => (e.id.clone(), e.thumbnail.clone()),
            None => (String::new(), None),
        };
        let art = self.art.get(ctx, &sel_id, sel_thumb.as_deref());
        let params = deck::DeckParams {
            art: art.as_ref(),
            disc_angle: self.disc_angle,
            spin: self.cfg.spin,
            arm: self.arm,
            needle: self.needle,
            power: playing,
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(16, 14, 12)))
            .show(ctx, |ui| {
                let avail = ui.available_size();
                let (rect, _) = ui.allocate_exact_size(avail, egui::Sense::hover());
                deck::draw(ui, rect, &params);
            });

        // Hamburger, visible only when the sidebar is closed
        // (the sidebar covers this corner and has its own close button).
        if !self.show_sidebar {
            egui::Area::new("hamburger".into())
                .fixed_pos([8.0, 8.0])
                .show(ctx, |ui| {
                    if ui
                        .button(egui::RichText::new("☰").size(18.0))
                        .clicked()
                    {
                        self.show_sidebar = true;
                        self.last_move = std::time::Instant::now();
                    }
                });
        }

        // Center play/stop overlay on hover.
        if controls_visible && !self.show_sidebar {
            let screen = ctx.screen_rect();
            let center = screen.center();
            egui::Area::new("transport".into())
                .fixed_pos(center - egui::vec2(60.0, 60.0))
                .show(ctx, |ui| {
                    ui.set_min_size(egui::vec2(120.0, 120.0));
                    ui.vertical_centered(|ui| {
                        let glyph = if playing { "⏸" } else { "▶" };
                        if ui
                            .button(egui::RichText::new(glyph).size(44.0))
                            .clicked()
                        {
                            self.toggle_play();
                        }
                    });
                });
        }

        // Opaque status bar along the bottom, hover-only like the transport.
        if controls_visible && !self.show_sidebar {
            egui::Area::new("statusbar".into())
                .anchor(egui::Align2::CENTER_BOTTOM, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgba_unmultiplied(10, 8, 6, 230))
                        .inner_margin(egui::Margin::symmetric(16, 8))
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(ctx.screen_rect().width(), 0.0));
                            ui.vertical_centered(|ui| {
                                ui.monospace(&self.status_text);
                            });
                        });
                });
        }

        // Sidebar overlay.
        if self.show_sidebar {
            let screen = ctx.screen_rect();
            let width = 300.0;
            let sidebar_rect =
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, screen.height()));
            egui::Area::new("sidebar".into())
                .fixed_pos([0.0, 0.0])
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgb(24, 20, 16))
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(width, screen.height()));
                            ui.set_max_size(egui::vec2(width, screen.height()));
                            // Larger type throughout the sidebar.
                            ui.style_mut().text_styles = [
                                (egui::TextStyle::Heading, egui::FontId::proportional(22.0)),
                                (egui::TextStyle::Body, egui::FontId::proportional(17.0)),
                                (egui::TextStyle::Button, egui::FontId::proportional(17.0)),
                                (egui::TextStyle::Small, egui::FontId::proportional(15.0)),
                                (egui::TextStyle::Monospace, egui::FontId::monospace(15.0)),
                            ]
                            .into();
                            // One continuous scroll for the whole sidebar.
                            egui::ScrollArea::vertical()
                                .max_height(screen.height())
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.heading("Streams");
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button("×").clicked() {
                                                    self.show_sidebar = false;
                                                }
                                                if ui.button("⟳").clicked() {
                                                    self.refresh_entries();
                                                }
                                            },
                                        );
                                    });
                                    if self.loading {
                                        ui.spinner();
                                    }
                                    // Full-width rows: justified layout stretches
                                    // each selectable label across the panel.
                                    ui.with_layout(
                                        egui::Layout::top_down_justified(egui::Align::LEFT),
                                        |ui| {
                                            let mut clicked: Option<usize> = None;
                                            for (i, e) in self.entries.iter().enumerate() {
                                                let dot = if e.is_live { "● " } else { "○ " };
                                                let label = format!("{dot}{}", e.title);
                                                let sel = self.selected == Some(i);
                                                if ui.selectable_label(sel, label).clicked() {
                                                    clicked = Some(i);
                                                }
                                                if let Some(v) = e.viewers {
                                                    ui.weak(format!("   {v} watching"));
                                                }
                                            }
                                            if let Some(i) = clicked {
                                                self.play_entry(i);
                                            }
                                        },
                                    );
                                    ui.separator();
                                    ui.label("Add stream:");
                                    ui.label("Name:");
                                    ui.text_edit_singleline(&mut self.new_name)
                                        .on_hover_text("display name for the stream");
                                    ui.label("URL:");
                                    ui.text_edit_singleline(&mut self.new_url)
                                        .on_hover_text("youtube video, live or channel URL");
                                    ui.horizontal(|ui| {
                                        if ui.button("＋ add").clicked()
                                            && !self.new_url.trim().is_empty()
                                        {
                                            let name = if self.new_name.trim().is_empty() {
                                                self.new_url.trim().to_string()
                                            } else {
                                                self.new_name.trim().to_string()
                                            };
                                            self.cfg.streams.push(UserStream {
                                                name,
                                                url: self.new_url.trim().to_string(),
                                            });
                                            let _ = config::save(&self.cfg);
                                            self.new_name.clear();
                                            self.new_url.clear();
                                            self.refresh_entries();
                                        }
                                        if ui.button("− remove").clicked() {
                                            if let Some(i) = self.selected {
                                                if let Some(entry) = self.entries.get(i) {
                                                    self.cfg.streams.retain(|s| s.url != entry.url);
                                                    if self.cfg.last_url.as_ref() == Some(&entry.url) {
                                                        self.cfg.last_url = None;
                                                    }
                                                    let _ = config::save(&self.cfg);
                                                    self.selected = None;
                                                    self.refresh_entries();
                                                }
                                            }
                                        }
                                    });
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        ui.label("vol");
                                        let mut vol = self.cfg.volume;
                                        if ui
                                            .add(egui::Slider::new(&mut vol, 0.0..=1.0))
                                            .changed()
                                        {
                                            self.cfg.volume = vol;
                                            self.player.set_volume(vol);
                                            let _ = config::save(&self.cfg);
                                        }
                                    });
                                    if ui.checkbox(&mut self.cfg.spin, "spin disc").changed() {
                                        let _ = config::save(&self.cfg);
                                    }
                                    ui.monospace(&self.status_text);
                                    ui.weak(format!("yt-dlp {}", self.ytdlp_version));
                                });
                        });
                });
            // Clicking outside the sidebar closes it. The click that opened it
            // lands on the (now hidden) hamburger inside the sidebar rect, so
            // it can't misfire here.
            let outside = ctx
                .pointer_hover_pos()
                .map(|p| !sidebar_rect.contains(p))
                .unwrap_or(false);
            if ctx.input(|i| i.pointer.any_click()) && outside {
                self.show_sidebar = false;
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}
