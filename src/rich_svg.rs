// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Bounded asynchronous SVG rasterization; zoom never runs a diagram engine.
use crate::egui;
use egui::load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};
use std::{
    collections::HashMap,
    sync::{Arc, mpsc},
};
type Key = (String, SizeHint);
enum Pixels {
    Pending,
    Ready(Arc<egui::ColorImage>),
    Error(String),
}
struct Entry {
    used: u64,
    pixels: Pixels,
}
type Cache = Arc<egui::mutex::Mutex<HashMap<Key, Entry>>>;
struct Task {
    key: Key,
    bytes: Arc<[u8]>,
    ctx: egui::Context,
}
pub struct SvgLoader {
    cache: Cache,
    tx: mpsc::SyncSender<Task>,
}
impl Default for SvgLoader {
    fn default() -> Self {
        let cache: Cache = Default::default();
        let shared = cache.clone();
        let (tx, tasks) = mpsc::sync_channel::<Task>(4);
        std::thread::spawn(move || {
            let mut options = resvg::usvg::Options::default();
            for font in [
                include_bytes!("../assets/MDcardDiagramSans-Regular.ttf").as_slice(),
                include_bytes!("../assets/NotoSansJP.ttf"),
                include_bytes!("../assets/MDcardDiagramSansKR-Regular.ttf"),
            ] {
                options.fontdb_mut().load_font_data(font.to_vec());
            }
            options
                .fontdb_mut()
                .set_sans_serif_family("MDcard Diagram Sans");
            options.font_family = "MDcard Diagram Sans".into();
            let mut trees = HashMap::<String, (usize, resvg::usvg::Tree)>::new();
            for task in tasks {
                if !shared.lock().contains_key(&task.key) {
                    continue;
                }
                let result = (|| -> Result<Arc<egui::ColorImage>, String> {
                    if !trees.contains_key(&task.key.0) {
                        let source = std::str::from_utf8(&task.bytes).map_err(|e| e.to_string())?;
                        if trees.len() >= 16
                            || trees.values().map(|(size, _)| *size).sum::<usize>() + source.len()
                                > 16 * 1024 * 1024
                        {
                            trees.clear();
                        }
                        let tree = resvg::usvg::Tree::from_str(source, &options)
                            .map_err(|e| e.to_string())?;
                        trees.insert(task.key.0.clone(), (source.len(), tree));
                    }
                    let tree = &trees[&task.key.0].1;
                    let original = egui::vec2(tree.size().width(), tree.size().height());
                    let mut size = match task.key.1 {
                        SizeHint::Size {
                            width,
                            height,
                            maintain_aspect_ratio,
                        } => {
                            let target = egui::vec2(width as f32, height as f32);
                            if maintain_aspect_ratio {
                                original * (target.x / original.x).min(target.y / original.y)
                            } else {
                                target
                            }
                        }
                        SizeHint::Width(w) => original * (w as f32 / original.x),
                        SizeHint::Height(h) => original * (h as f32 / original.y),
                        SizeHint::Scale(s) => original * s.into_inner(),
                    };
                    size *= (4096. / size.x)
                        .min(4096. / size.y)
                        .min((8_000_000. / (size.x * size.y)).sqrt())
                        .min(1.);
                    let width = size.x.round().max(1.) as u32;
                    let height = size.y.round().max(1.) as u32;
                    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
                        .ok_or("SVG canvas allocation failed")?;
                    resvg::render(
                        tree,
                        resvg::usvg::Transform::from_scale(
                            width as f32 / original.x,
                            height as f32 / original.y,
                        ),
                        &mut pixmap.as_mut(),
                    );
                    Ok(Arc::new(
                        egui::ColorImage::from_rgba_premultiplied(
                            [width as usize, height as usize],
                            pixmap.data(),
                        )
                        .with_source_size(original),
                    ))
                })();
                let mut cache = shared.lock();
                if cache.contains_key(&task.key) {
                    let size = result.as_ref().map_or(0, |image| image.pixels.len() * 4);
                    while byte_size(&cache) + size > 64 * 1024 * 1024 {
                        let victim = cache
                            .iter()
                            .filter(|(key, _)| *key != &task.key)
                            .min_by_key(|(_, entry)| entry.used)
                            .map(|(key, _)| key.clone());
                        if let Some(key) = victim {
                            cache.remove(&key);
                        } else {
                            break;
                        }
                    }
                    cache.get_mut(&task.key).unwrap().pixels = match result {
                        Ok(image) => Pixels::Ready(image),
                        Err(e) => Pixels::Error(e),
                    };
                }
                drop(cache);
                task.ctx.request_repaint();
            }
        });
        Self { cache, tx }
    }
}
fn byte_size(cache: &HashMap<Key, Entry>) -> usize {
    cache
        .values()
        .map(|entry| match &entry.pixels {
            Pixels::Ready(image) => image.pixels.len() * 4,
            _ => 0,
        })
        .sum()
}
impl ImageLoader for SvgLoader {
    fn id(&self) -> &str {
        "mdcard::rich-svg"
    }
    fn load(&self, ctx: &egui::Context, uri: &str, hint: SizeHint) -> ImageLoadResult {
        if !uri.starts_with("bytes://rich-") || !uri.ends_with(".svg") {
            return Err(LoadError::NotSupported);
        }
        let key = (uri.to_string(), hint);
        let pass = ctx.cumulative_pass_nr();
        if let Some(entry) = self.cache.lock().get_mut(&key) {
            entry.used = pass;
            return match &entry.pixels {
                Pixels::Ready(image) => Ok(ImagePoll::Ready {
                    image: image.clone(),
                }),
                Pixels::Pending => Ok(ImagePoll::Pending { size: None }),
                Pixels::Error(message) => Err(LoadError::Loading(message.clone())),
            };
        }
        match ctx.try_load_bytes(uri)? {
            BytesPoll::Pending { size } => Ok(ImagePoll::Pending { size }),
            BytesPoll::Ready { bytes, .. } => {
                let mut cache = self.cache.lock();
                cache.insert(
                    key.clone(),
                    Entry {
                        used: pass,
                        pixels: Pixels::Pending,
                    },
                );
                if self
                    .tx
                    .try_send(Task {
                        key: key.clone(),
                        bytes: bytes.as_ref().to_vec().into(),
                        ctx: ctx.clone(),
                    })
                    .is_err()
                {
                    cache.remove(&key);
                    ctx.request_repaint_after(std::time::Duration::from_millis(16));
                }
                Ok(ImagePoll::Pending { size: None })
            }
        }
    }
    fn forget(&self, uri: &str) {
        self.cache.lock().retain(|(key, _), _| key != uri);
    }
    fn forget_all(&self) {
        self.cache.lock().clear();
    }
    fn byte_size(&self) -> usize {
        byte_size(&self.cache.lock())
    }
    fn end_pass(&self, pass: u64) {
        self.cache
            .lock()
            .retain(|_, entry| pass.saturating_sub(entry.used) < 2);
    }
}
