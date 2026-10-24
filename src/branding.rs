// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Embedded application identity. Runtime never depends on an unpacked asset path.
use eframe::egui::{
    self, Color32, ColorImage, IconData, Response, TextureHandle, TextureOptions, Vec2,
};

const SIZES: [u32; 9] = [16, 24, 32, 48, 64, 128, 256, 512, 1024];
macro_rules! png_set {
    ($variant:literal) => {
        [
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-16.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-24.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-32.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-48.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-64.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-128.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-256.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-512.png"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../assets/branding/mdcard-icon/png/",
                $variant,
                "/mdcard-1024.png"
            ))
            .as_slice(),
        ]
    };
}
static COLOR: [&[u8]; 9] = png_set!("color");
static BLACK: [&[u8]; 9] = png_set!("black");
static WHITE: [&[u8]; 9] = png_set!("white");

fn selected_index(physical_pixels: f32) -> usize {
    let wanted = if physical_pixels.is_finite() {
        physical_pixels.max(1.0)
    } else {
        24.0
    };
    SIZES
        .iter()
        .position(|&size| size as f32 >= wanted)
        .unwrap_or(SIZES.len() - 1)
}

fn decode(bytes: &[u8], color: [u8; 3]) -> (image::RgbaImage, bool) {
    match image::load_from_memory(bytes) {
        Ok(image) => (image.to_rgba8(), false),
        Err(_) => (fallback(color), true),
    }
}

// Small, dependency-free geometric recovery if a future embedded raster is invalid.
fn fallback(color: [u8; 3]) -> image::RgbaImage {
    let points = [
        (20.0f32, 43.0f32),
        (20.0, 22.0),
        (32.0, 34.0),
        (44.0, 22.0),
        (44.0, 43.0),
    ];
    image::RgbaImage::from_fn(64, 64, |x, y| {
        let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
        let dx = (x - 32.0).abs() - 12.0;
        let dy = (y - 32.0).abs() - 16.0;
        let card = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) <= 12.0;
        let cut = points.windows(2).any(|segment| {
            let ((ax, ay), (bx, by)) = (segment[0], segment[1]);
            let (vx, vy) = (bx - ax, by - ay);
            let t = (((x - ax) * vx + (y - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
            (x - ax - t * vx).hypot(y - ay - t * vy) <= 3.75
        });
        image::Rgba([
            color[0],
            color[1],
            color[2],
            if card && !cut { 255 } else { 0 },
        ])
    })
}

pub fn window_icon() -> IconData {
    let (image, _) = decode(COLOR[6], [45, 91, 202]);
    IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

pub fn brand(ui: &mut egui::Ui, background: Color32) -> Response {
    // Choose from the pixels actually behind the logo, including theme transitions.
    let [r, g, b, _] = background.to_array();
    let dark = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) < 128_000;
    let (variant, sources, ink) = if dark {
        ("white", &WHITE, [255, 255, 255])
    } else {
        ("black", &BLACK, [0, 0, 0])
    };
    let index = selected_index(24.0 * ui.ctx().pixels_per_point());
    let key = egui::Id::new(("mdcard-brand", variant, SIZES[index]));
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<(TextureHandle, bool)>(key));
    let (texture, recovered) = cached.unwrap_or_else(|| {
        let (image, recovered) = decode(sources[index], ink);
        let texture = ui.ctx().load_texture(
            format!("mdcard-{variant}-{}", SIZES[index]),
            ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            ),
            TextureOptions::LINEAR,
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(key, (texture.clone(), recovered)));
        (texture, recovered)
    });
    let response =
        ui.add(egui::Image::new((texture.id(), Vec2::splat(24.0))).sense(egui::Sense::hover()));
    if std::env::var_os("MDCARD_UI_REPORT").is_some() {
        let rect = response.rect;
        ui.ctx().data_mut(|data|data.insert_temp(egui::Id::new("mdcard-brand-drawn"),serde_json::json!({"variant":variant,"resource_size":SIZES[index],"logical_size":24,"background_rgba":background.to_array(),"fallback":recovered,"rect":[rect.min.x,rect.min.y,rect.max.x,rect.max.y]})));
    }
    response.on_hover_text("MDcard")
}

pub fn set_process_identity() {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(id: *const u16) -> i32;
        }
        let id: Vec<u16> = "MDcard.Desktop".encode_utf16().chain(Some(0)).collect();
        // A failed optional shell grouping request must not prevent document access.
        let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(id.as_ptr()) };
    }
}

#[derive(Default)]
pub struct NativeIcons {
    #[cfg(target_os = "windows")]
    icons: Option<(isize, u32, isize, isize, i32, i32)>,
}

impl NativeIcons {
    pub fn update(&mut self, _frame: &eframe::Frame) {
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let Ok(handle) = _frame.window_handle() else {
                return;
            };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return;
            };
            let hwnd = handle.hwnd.get();
            // eframe owns this live HWND and invokes update on its UI thread.
            unsafe {
                let dpi = native::GetDpiForWindow(hwnd).max(96);
                if self
                    .icons
                    .is_some_and(|icons| icons.0 == hwnd && icons.1 == dpi)
                {
                    return;
                }
                let big_size = native::GetSystemMetricsForDpi(11, dpi);
                let small_size = native::GetSystemMetricsForDpi(49, dpi);
                let module = native::GetModuleHandleW(std::ptr::null());
                // Resource 1 contains every optical ICO size; LoadImage selects its source.
                // MAKEINTRESOURCEW(1) encodes an integer ID, not a memory pointer.
                let resource_id = std::ptr::without_provenance::<u16>(1);
                let big = native::LoadImageW(module, resource_id, 1, big_size, big_size, 0);
                let small = native::LoadImageW(module, resource_id, 1, small_size, small_size, 0);
                if big == 0 || small == 0 {
                    if big != 0 {
                        native::DestroyIcon(big);
                    }
                    if small != 0 {
                        native::DestroyIcon(small);
                    }
                    return;
                }
                native::SendMessageW(hwnd, 0x80, 1, big);
                native::SendMessageW(hwnd, 0x80, 0, small);
                if let Some(previous) = self
                    .icons
                    .replace((hwnd, dpi, big, small, big_size, small_size))
                {
                    // Only destroy our allocated handles; winit owns its initial fallback.
                    native::DestroyIcon(previous.2);
                    native::DestroyIcon(previous.3);
                }
            }
        }
    }

    pub fn report(&self) -> serde_json::Value {
        #[cfg(target_os = "windows")]
        {
            self.icons.map(|icons|serde_json::json!({"dpi":icons.1,"big_pixels":icons.4,"small_pixels":icons.5,"source":"PE resource 1, LoadImageW size selection","big_handle":icons.2,"small_handle":icons.3})).unwrap_or(serde_json::Value::Null)
        }
        #[cfg(not(target_os = "windows"))]
        {
            serde_json::Value::Null
        }
    }
}

impl Drop for NativeIcons {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if let Some((hwnd, _, big, small, _, _)) = self.icons.take() {
            unsafe {
                if native::IsWindow(hwnd) != 0 {
                    native::SendMessageW(hwnd, 0x80, 1, 0);
                    native::SendMessageW(hwnd, 0x80, 0, 0);
                }
                native::DestroyIcon(big);
                native::DestroyIcon(small);
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod native {
    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetDpiForWindow(hwnd: isize) -> u32;
        pub fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
        pub fn LoadImageW(
            module: isize,
            name: *const u16,
            kind: u32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> isize;
        pub fn SendMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
        pub fn DestroyIcon(icon: isize) -> i32;
        pub fn IsWindow(hwnd: isize) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetModuleHandleW(name: *const u16) -> isize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_embedded_variants_decode_with_matching_transparency() {
        for (index, size) in SIZES.into_iter().enumerate() {
            let mut reference = None;
            for (bytes, color) in [
                (COLOR[index], [45, 91, 202]),
                (BLACK[index], [0, 0, 0]),
                (WHITE[index], [255, 255, 255]),
            ] {
                let (image, recovered) = decode(bytes, color);
                assert!(!recovered);
                assert_eq!(image.dimensions(), (size, size));
                assert_eq!(image.get_pixel(0, 0)[3], 0);
                assert_eq!(image.get_pixel(size / 2, size / 2)[3], 0);
                let alpha: Vec<_> = image.pixels().map(|pixel| pixel[3]).collect();
                if let Some(expected) = &reference {
                    assert_eq!(&alpha, expected)
                } else {
                    reference = Some(alpha)
                }
            }
        }
    }
    #[test]
    fn corrupt_image_has_visible_transparent_geometric_fallback() {
        let (image, recovered) = decode(b"corrupt", [255, 255, 255]);
        assert!(recovered);
        assert_eq!(image.get_pixel(0, 0)[3], 0);
        assert_eq!(image.get_pixel(32, 32)[3], 0);
        assert!(image.pixels().any(|pixel| pixel[3] == 255));
    }
    #[test]
    fn dpi_selection_preserves_optical_sources_and_is_bounded() {
        for (pixels, size) in [
            (16.0, 16),
            (24.0, 24),
            (30.0, 32),
            (48.0, 48),
            (64.0, 64),
            (2000.0, 1024),
            (f32::NAN, 24),
        ] {
            assert_eq!(SIZES[selected_index(pixels)], size);
        }
    }
}
