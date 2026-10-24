// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Let the Windows compositor clip the native window, keeping both renderers opaque.
//! Linux decoration/rounding policy belongs to the user's window manager/compositor.

#[derive(Default)]
pub struct NativeCorners {
    #[cfg(target_os = "windows")]
    last_state: Option<(isize, bool)>,
}

impl NativeCorners {
    pub fn update(&mut self, _frame: &eframe::Frame, _maximized: bool) {
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
            if self.last_state == Some((hwnd, _maximized)) {
                return;
            }
            self.last_state = Some((hwnd, _maximized));
            let preference: u32 = if _maximized { 1 } else { 2 };
            // SAFETY: eframe owns the live HWND and invokes update on its UI thread.
            // DWM copies this four-byte enum during the call; no pointer is retained.
            // Unsupported systems simply retain their native rectangular window.
            unsafe {
                DwmSetWindowAttribute(
                    hwnd as *mut std::ffi::c_void,
                    33, // DWMWA_WINDOW_CORNER_PREFERENCE
                    std::ptr::from_ref(&preference).cast(),
                    std::mem::size_of_val(&preference) as u32,
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: *mut std::ffi::c_void,
        attribute: u32,
        value: *const std::ffi::c_void,
        size: u32,
    ) -> i32;
}
