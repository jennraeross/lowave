use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Square cover, circular-masked disc label, and wide J-card art for one video.
#[derive(Clone)]
pub struct Art {
    pub square: egui::TextureHandle,
    pub disc: egui::TextureHandle,
    pub wide: egui::TextureHandle,
}

/// Downloads YouTube thumbnails in the background and hands out egui
/// textures. Files are cached under the config dir as `art/<videoid>.png`
/// (16:9, unmasked) so refreshes are instant and offline-friendly.
pub struct ArtCache {
    dir: PathBuf,
    mem: HashMap<String, Art>,
    inflight: Arc<Mutex<HashSet<String>>>,
}

impl ArtCache {
    pub fn new() -> Self {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lowave")
            .join("art");
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            mem: HashMap::new(),
            inflight: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Non-blocking lookup. Returns `None` until the art is downloaded and
    /// uploaded; triggers repaint when it arrives.
    pub fn get(&mut self, ctx: &egui::Context, id: &str, url: Option<&str>) -> Option<Art> {
        if id.is_empty() {
            return None;
        }
        if let Some(a) = self.mem.get(id) {
            return Some(a.clone());
        }
        let path = self.dir.join(format!("{id}.png"));
        if path.is_file() {
            if let Some(a) = Self::load_textures(ctx, id, &path) {
                self.mem.insert(id.to_string(), a.clone());
                return Some(a);
            }
        }
        if let Some(u) = url {
            let mut inf = self.inflight.lock().unwrap();
            if inf.insert(id.to_string()) {
                let id_owned = id.to_string();
                let url_owned = u.to_string();
                let ctx2 = ctx.clone();
                let inflight = self.inflight.clone();
                std::thread::spawn(move || {
                    Self::download(&url_owned, &path);
                    inflight.lock().unwrap().remove(&id_owned);
                    ctx2.request_repaint();
                });
            }
        }
        None
    }

    /// Fit to 16:9 and save unmasked, so the same file serves the upright
    /// cover, the masked spinning disc, and the undistorted J-card label.
    fn download(url: &str, path: &PathBuf) {
        let bytes = match reqwest::blocking::get(url).and_then(|r| r.bytes()) {
            Ok(b) => b,
            Err(_) => return,
        };
        let img = match image::load_from_memory(&bytes) {
            Ok(i) => i.to_rgba8(),
            Err(_) => return,
        };
        // Fit inside 512x288 preserving ratio, letterboxed on black so the
        // cached file is always exactly 16:9.
        let fitted = image::imageops::thumbnail(&img, 512, 288);
        let mut out = image::RgbaImage::from_pixel(512, 288, image::Rgba([0, 0, 0, 255]));
        let x = (512 - fitted.width()) / 2;
        let y = (288 - fitted.height()) / 2;
        image::imageops::overlay(&mut out, &fitted, x as i64, y as i64);
        let _ = out.save(path);
    }

    fn upload(
        ctx: &egui::Context,
        name: String,
        img: &image::RgbaImage,
    ) -> egui::TextureHandle {
        let (w, h) = (img.width() as usize, img.height() as usize);
        let pixels: Vec<egui::Color32> = img
            .pixels()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        ctx.load_texture(
            name,
            egui::ColorImage {
                size: [w, h],
                source_size: egui::Vec2::new(w as f32, h as f32),
                pixels,
            },
            egui::TextureOptions::LINEAR,
        )
    }

    /// Builds all three textures: square cover (center-cropped), disc with a
    /// baked circular alpha mask (rotation turns off rounding, so the mask
    /// must be baked), and the wide J-card art as-is.
    fn load_textures(ctx: &egui::Context, id: &str, path: &PathBuf) -> Option<Art> {
        let wide_img = image::open(path).ok()?.to_rgba8();
        // Stale cache (old square/masked saves): re-download.
        if wide_img.width() != 512 || wide_img.height() != 288 {
            let _ = std::fs::remove_file(path);
            return None;
        }
        let square_img = image::imageops::crop_imm(&wide_img, 112, 16, 256, 256).to_image();
        let mut disc_img = square_img.clone();
        let size = disc_img.width() as f32;
        let r = size / 2.0;
        for (px, py, pixel) in disc_img.enumerate_pixels_mut() {
            let dx = px as f32 - r + 0.5;
            let dy = py as f32 - r + 0.5;
            let d = (dx * dx + dy * dy).sqrt();
            if d > r {
                pixel[3] = 0;
            } else if d > r - 1.0 {
                let a = (r - d).clamp(0.0, 1.0);
                pixel[3] = (pixel[3] as f32 * a) as u8;
            }
        }
        Some(Art {
            square: Self::upload(ctx, format!("art-{id}-sq"), &square_img),
            disc: Self::upload(ctx, format!("art-{id}-disc"), &disc_img),
            wide: Self::upload(ctx, format!("art-{id}-wide"), &wide_img),
        })
    }
}
