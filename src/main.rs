#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod cassette;
mod config;
mod deck;
mod playback;
mod playlist;
mod ytdlp;

use art::ArtCache;
use config::{AppConfig, UserPlaylist, UserStream, ViewMode};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideTab {
    Streams,
    Playlists,
}

/// In-progress playlist being built/edited. Saved to disk as .m3u, then
/// takes the normal import path (config + re-parse).
struct PlaylistDraft {
    name: String,
    entries: Vec<playlist::DraftEntry>,
    /// Existing file when editing an imported playlist (save overwrites it).
    edit_path: Option<std::path::PathBuf>,
    new_url: String,
    new_title: String,
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
    play_time: f32,
    side_tab: SideTab,
    playlists: Vec<playlist::PlaylistFile>,
    draft: Option<PlaylistDraft>,
    new_name: String,
    new_url: String,
    loading: bool,
    show_sidebar: bool,
    last_frame: std::time::Instant,
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
            play_time: 0.0,
            side_tab: SideTab::Streams,
            playlists: Vec::new(),
            draft: None,
            new_name: String::new(),
            new_url: String::new(),
            loading: true,
            show_sidebar: false,
            last_frame: now,
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
        // Then imported .m3u playlists, re-parsed from disk.
        let mut pls = Vec::new();
        for pl in &self.cfg.playlists {
            let parsed =
                playlist::load_m3u(&pl.name, &std::path::PathBuf::from(&pl.path));
            out.extend(parsed.entries.iter().cloned());
            pls.push(parsed);
        }
        self.playlists = pls;
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
            self.play_time = 0.0;
            self.player.play(e.title.clone(), e.media.clone());
        }
    }

    /// Import .m3u files as named playlists (dedupes by path).
    fn import_playlists(&mut self, paths: Vec<std::path::PathBuf>) {
        let mut changed = false;
        for path in paths {
            let is_m3u = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
                .unwrap_or(false);
            if !is_m3u || !path.is_file() {
                continue;
            }
            let ps = path.to_string_lossy().to_string();
            if self.cfg.playlists.iter().any(|p| p.path == ps) {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("playlist")
                .to_string();
            self.cfg.playlists.push(UserPlaylist { name, path: ps });
            changed = true;
        }
        if changed {
            let _ = config::save(&self.cfg);
            self.refresh_entries();
        }
    }

    /// Ensure a draft exists, then add a stream entry to it and jump to the
    /// Playlists tab so the editor is visible.
    fn add_stream_to_draft(&mut self, i: usize) {
        if let Some(e) = self.entries.get(i) {
            let draft = self.draft.get_or_insert_with(|| PlaylistDraft {
                name: "New playlist".to_string(),
                entries: Vec::new(),
                edit_path: None,
                new_url: String::new(),
                new_title: String::new(),
            });
            draft.entries.push(playlist::DraftEntry::new(
                e.title.clone(),
                e.url.clone(),
            ));
            self.side_tab = SideTab::Playlists;
        }
    }

    /// Load an imported playlist file back into the draft for editing.
    fn edit_playlist(&mut self, name: &str, path: &std::path::Path) {
        let parsed = playlist::load_m3u(name, path);
        self.draft = Some(PlaylistDraft {
            name: name.to_string(),
            entries: parsed
                .entries
                .iter()
                .map(|e| playlist::DraftEntry::new(e.title.clone(), e.url.clone()))
                .collect(),
            edit_path: Some(path.to_path_buf()),
            new_url: String::new(),
            new_title: String::new(),
        });
    }

    /// Write the draft to disk (.m3u) and import the result.
    fn save_draft(&mut self, path: std::path::PathBuf) {
        if let Some(d) = &self.draft {
            if playlist::write_m3u(&path, &d.entries).is_ok() {
                let ps = path.to_string_lossy().to_string();
                if !self.cfg.playlists.iter().any(|p| p.path == ps) {
                    self.cfg.playlists.push(UserPlaylist {
                        name: d.name.clone(),
                        path: ps,
                    });
                    let _ = config::save(&self.cfg);
                }
                self.draft = None;
                self.refresh_entries();
            }
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

    /// Streams tab: YouTube entries + add/remove.
    fn streams_tab(&mut self, ui: &mut egui::Ui) {
        // Full-width rows: justified layout stretches
        // each selectable label across the panel.
        ui.with_layout(
            egui::Layout::top_down_justified(egui::Align::LEFT),
            |ui| {
                let mut clicked: Option<usize> = None;
                let mut add_to_draft: Option<usize> = None;
                for (i, e) in self.entries.iter().enumerate() {
                    if e.playlist.is_some() {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        let dot = if e.is_live { "● " } else { "○ " };
                        let label = format!("{dot}{}", e.title);
                        let sel = self.selected == Some(i);
                        if ui.selectable_label(sel, label).clicked() {
                            clicked = Some(i);
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button("＋").on_hover_text("add to playlist draft").clicked() {
                                    add_to_draft = Some(i);
                                }
                            },
                        );
                    });
                    if let Some(v) = e.viewers {
                        ui.weak(format!("   {v} watching"));
                    }
                }
                if let Some(i) = clicked {
                    self.play_entry(i);
                }
                if let Some(i) = add_to_draft {
                    self.add_stream_to_draft(i);
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
            if ui.button("＋ add").clicked() && !self.new_url.trim().is_empty() {
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
                    if let Some(entry) = self.entries.get(i).filter(|e| e.playlist.is_none()) {
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
    }

    /// Playlists tab: imported .m3u files, grouped with their entries —
    /// or the builder editor when a draft is open.
    fn playlists_tab(&mut self, ui: &mut egui::Ui) {
        if self.draft.is_some() {
            self.draft_editor(ui);
            return;
        }
        ui.horizontal(|ui| {
            if ui.button("＋ import .m3u").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("playlists", &["m3u", "m3u8"])
                    .pick_file()
                {
                    self.import_playlists(vec![path]);
                }
            }
            if ui.button("＋ new playlist").clicked() {
                self.draft = Some(PlaylistDraft {
                    name: "New playlist".to_string(),
                    entries: Vec::new(),
                    edit_path: None,
                    new_url: String::new(),
                    new_title: String::new(),
                });
            }
            if ui.button("⟳").clicked() {
                self.refresh_entries();
            }
        });
        ui.weak("tip: drag & drop .m3u files anywhere");
        if self.playlists.is_empty() {
            ui.weak("no playlists yet");
            return;
        }
        let mut remove: Option<String> = None;
        let mut edit: Option<(String, std::path::PathBuf)> = None;
        let mut clicked: Option<usize> = None;
        for pl in &self.playlists {
            ui.horizontal(|ui| {
                ui.heading(pl.name.clone());
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui.small_button("remove").clicked() {
                            remove = Some(pl.path.to_string_lossy().to_string());
                        }
                        if ui.small_button("edit").clicked() {
                            edit = Some((pl.name.clone(), pl.path.clone()));
                        }
                    },
                );
            });
            if pl.missing > 0 {
                ui.weak(format!("{} file(s) missing", pl.missing));
            }
            ui.with_layout(
                egui::Layout::top_down_justified(egui::Align::LEFT),
                |ui| {
                    for (i, e) in self.entries.iter().enumerate() {
                        if e.playlist.as_deref() != Some(pl.name.as_str()) {
                            continue;
                        }
                        if ui.selectable_label(self.selected == Some(i), &e.title).clicked() {
                            clicked = Some(i);
                        }
                    }
                },
            );
        }
        if let Some(i) = clicked {
            self.play_entry(i);
        }
        if let Some((name, path)) = edit {
            self.edit_playlist(&name, &path);
        }
        if let Some(path) = remove {
            self.cfg.playlists.retain(|p| p.path != path);
            if let Some(i) = self.selected {
                if self
                    .entries
                    .get(i)
                    .and_then(|e| e.playlist.clone())
                    .is_some()
                {
                    self.selected = None;
                }
            }
            let _ = config::save(&self.cfg);
            self.refresh_entries();
        }
    }

    /// Builder editor: rename, reorder/remove entries, add URL or files,
    /// then save to .m3u (overwrites when editing, save-dialog otherwise).
    fn draft_editor(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Playlist builder");
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui.small_button("cancel").clicked() {
                        self.draft = None;
                    }
                },
            );
        });
        ui.label("Name:");
        let mut save_to: Option<std::path::PathBuf> = None;
        let mut cancel = false;
        if let Some(d) = self.draft.as_mut() {
            ui.text_edit_singleline(&mut d.name);
            let n = d.entries.len();
            let mut mv: Option<(usize, i32)> = None;
            let mut rm: Option<usize> = None;
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for (i, e) in d.entries.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}. {}", i + 1, e.title));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("×").clicked() {
                                        rm = Some(i);
                                    }
                                    if ui.small_button("↓").clicked() {
                                        mv = Some((i, 1));
                                    }
                                    if ui.small_button("↑").clicked() {
                                        mv = Some((i, -1));
                                    }
                                },
                            );
                        });
                        ui.weak(&e.target);
                    }
                });
            if n == 0 {
                ui.weak("empty — add streams with ＋, files, or a URL below");
            }
            ui.separator();
            ui.label("Add URL:");
            ui.text_edit_singleline(&mut d.new_url)
                .on_hover_text("youtube or direct audio URL");
            ui.label("Title (optional):");
            ui.text_edit_singleline(&mut d.new_title);
            ui.horizontal(|ui| {
                let url_ok = !d.new_url.trim().is_empty();
                if ui.add_enabled(url_ok, egui::Button::new("＋ add URL")).clicked() {
                    let target = d.new_url.trim().to_string();
                    let title = if d.new_title.trim().is_empty() {
                        target.clone()
                    } else {
                        d.new_title.trim().to_string()
                    };
                    d.entries.push(playlist::DraftEntry::new(title, target));
                    d.new_url.clear();
                    d.new_title.clear();
                }
                if ui.button("＋ add files").clicked() {
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter(
                            "audio",
                            &["mp3", "m4a", "flac", "wav", "ogg", "opus", "aac", "wma"],
                        )
                        .pick_files()
                    {
                        for p in paths {
                            let target = p.to_string_lossy().to_string();
                            let title = p
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("track")
                                .to_string();
                            d.entries.push(playlist::DraftEntry::new(title, target));
                        }
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let can_save = !d.name.trim().is_empty() && !d.entries.is_empty();
                if ui.add_enabled(can_save, egui::Button::new("save")).clicked() {
                    if let Some(p) = d.edit_path.clone() {
                        save_to = Some(p);
                    } else if let Some(p) = rfd::FileDialog::new()
                        .set_file_name(format!("{}.m3u", d.name.trim()))
                        .add_filter("m3u playlist", &["m3u"])
                        .save_file()
                    {
                        save_to = Some(p);
                    }
                }
                if ui.small_button("cancel").clicked() {
                    cancel = true;
                }
            });
            if let Some((i, dir)) = mv {
                let j = (i as i32 + dir).clamp(0, n as i32 - 1) as usize;
                if j != i {
                    d.entries.swap(i, j);
                }
            }
            if let Some(i) = rm {
                d.entries.remove(i);
            }
        }
        if let Some(p) = save_to {
            self.save_draft(p);
        }
        if cancel {
            self.draft = None;
        }
    }

    fn step_stream(&mut self, dir: i32) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as i32;
        let cur = self.selected.unwrap_or(0) as i32;
        let next = (cur + dir).rem_euclid(len) as usize;
        self.play_entry(next);
    }
}

impl eframe::App for LowaveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        // Drag-and-drop .m3u import.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.import_playlists(dropped);
        }

        let st = self.player.status();
        let playing = matches!(st, PlaybackStatus::Playing(_));        self.status_text = match &st {
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
            self.play_time += dt;
        }

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
        let view = self.cfg.view;
        let params = deck::DeckParams {
            art: art.as_ref(),
            disc_angle: self.disc_angle,
            spin: self.cfg.spin,
            arm: self.arm,
            needle: self.needle,
            power: playing,
            volume: self.cfg.volume,
        };
        let c_params = cassette::CassetteParams {
            art: art.as_ref(),
            play_time: self.play_time,
            playing,
            power: playing,
            volume: self.cfg.volume,
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(16, 14, 12)))
            .show(ctx, |ui| {
                let avail = ui.available_size();
                let (rect, _) = ui.allocate_exact_size(avail, egui::Sense::hover());
                let action = match view {
                    ViewMode::Vinyl => deck::draw(ui, rect, &params),
                    ViewMode::Cassette => cassette::draw(ui, rect, &c_params),
                };
                if action.toggle_play {
                    self.toggle_play();
                }
                if action.stop {
                    self.player.stop();
                }
                if action.next {
                    self.step_stream(1);
                }
                if action.prev {
                    self.step_stream(-1);
                }
                if let Some(v) = action.volume {
                    self.cfg.volume = v;
                    self.player.set_volume(v);
                    let _ = config::save(&self.cfg);
                }
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
                    }
                });
        }

        // Opaque status bar along the bottom, hover-only.

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
                                        ui.heading("Library");
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
                                    // Tabs: streams vs imported .m3u playlists.
                                    ui.horizontal(|ui| {
                                        if ui
                                            .selectable_label(
                                                self.side_tab == SideTab::Streams,
                                                "Streams",
                                            )
                                            .clicked()
                                        {
                                            self.side_tab = SideTab::Streams;
                                        }
                                        if ui
                                            .selectable_label(
                                                self.side_tab == SideTab::Playlists,
                                                "Playlists",
                                            )
                                            .clicked()
                                        {
                                            self.side_tab = SideTab::Playlists;
                                        }
                                    });
                                    match self.side_tab {
                                        SideTab::Streams => self.streams_tab(ui),
                                        SideTab::Playlists => self.playlists_tab(ui),
                                    }
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
                                    if self.cfg.view == ViewMode::Vinyl
                                        && ui.checkbox(&mut self.cfg.spin, "spin disc").changed()
                                    {
                                        let _ = config::save(&self.cfg);
                                    }
                                    if ui
                                        .button(format!("view: {}", self.cfg.view.label()))
                                        .clicked()
                                    {
                                        self.cfg.view = self.cfg.view.toggle();
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
