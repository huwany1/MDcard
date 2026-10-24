// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use eframe::egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};
use std::sync::Arc;

pub fn software_setup(language: crate::localization::Language) -> WgpuSetup {
    let mut setup = WgpuSetupCreateNew::default();
    // Do not consult WGPU_BACKEND: disabling acceleration must never enable a GPU.
    setup.instance_descriptor.backends = if cfg!(target_os = "windows") {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::VULKAN
    };
    // Fxc is provided by Windows; no separately downloaded DXC DLL is required.
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .shader_compiler = wgpu::Dx12Compiler::Fxc;
    setup.native_adapter_selector = Some(Arc::new(move |adapters, surface| {
        adapters.iter().find(|a| a.get_info().device_type == wgpu::DeviceType::Cpu && surface.is_none_or(|s| a.is_surface_supported(s)))
            .cloned().ok_or_else(|| language.text("No compatible CPU renderer was found. GPU acceleration remains disabled. On Linux install mesa-vulkan-drivers and libvulkan1. To recover, launch MDcard with --gpu on (this explicitly updates the saved preference).").into())
    }));
    setup.into()
}

pub fn description(cc: &eframe::CreationContext<'_>) -> String {
    if let Some(state) = &cc.wgpu_render_state {
        let i = state.adapter.get_info();
        return format!(
            "Software · {} · {:?} / {:?}",
            i.name, i.device_type, i.backend
        );
    }
    if let Some(gl) = &cc.gl {
        use eframe::glow::HasContext;
        // SAFETY: eframe supplies the current, live OpenGL context on this thread.
        return unsafe {
            format!(
                "OpenGL · {} · {}",
                gl.get_parameter_string(eframe::glow::RENDERER),
                gl.get_parameter_string(eframe::glow::VERSION)
            )
        };
    }
    "Renderer unavailable".into()
}

/// Benchmark-only tail callback. Normal product rendering never calls glFinish.
pub fn measure_completion(
    ctx: &eframe::egui::Context,
    shared: Arc<std::sync::Mutex<crate::large_document::PaintMetrics>>,
    frame_start: Option<std::time::Instant>,
    opened: Option<std::time::Instant>,
    input: bool,
    revision: u64,
) {
    if std::env::var_os("MDCARD_PERFORMANCE_REPORT").is_none() {
        return;
    }
    let gpu_finish = std::env::var_os("MDCARD_PERFORMANCE_GPU_FINISH").is_some();
    let callback = eframe::egui_glow::CallbackFn::new(move |_, painter| {
        if gpu_finish {
            use eframe::glow::HasContext; // SAFETY: callback runs with the live context current.
            unsafe {
                painter.gl().finish();
            }
        }
        let now = std::time::Instant::now();
        let mut samples = shared.lock().unwrap();
        samples.gpu_finish = gpu_finish;
        #[cfg(windows)]
        if input {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn QueryPerformanceCounter(value: *mut i64) -> i32;
                fn QueryPerformanceFrequency(value: *mut i64) -> i32;
            }
            let (mut counter, mut frequency) = (0i64, 0i64);
            // SAFETY: both Windows APIs write one valid stack-allocated i64.
            if unsafe {
                QueryPerformanceCounter(&mut counter) != 0
                    && QueryPerformanceFrequency(&mut frequency) != 0
            } {
                samples.qpc_frequency = frequency as u64;
                if samples.input_completions.len() >= 4096 {
                    samples.input_completions.pop_front();
                }
                samples
                    .input_completions
                    .push_back(serde_json::json!({"revision": revision, "qpc": counter as u64}));
            }
        }
        #[cfg(not(windows))]
        let _ = revision;
        samples.frames += 1;
        fn push(q: &mut std::collections::VecDeque<f64>, v: f64) {
            if q.len() >= 4096 {
                q.pop_front();
            }
            q.push_back(v);
        }
        if let Some(last) = samples.last {
            push(
                &mut samples.intervals_ms,
                now.duration_since(last).as_secs_f64() * 1000.,
            );
        }
        samples.last = Some(now);
        if samples.first_complete_ms.is_none() {
            samples.first_complete_ms =
                opened.map(|at| now.duration_since(at).as_secs_f64() * 1000.);
        }
        if let Some(start) = frame_start {
            let ms = now.duration_since(start).as_secs_f64() * 1000.;
            push(&mut samples.frame_to_complete_ms, ms);
            if input {
                push(&mut samples.input_to_complete_ms, ms);
            }
        }
    });
    ctx.layer_painter(eframe::egui::LayerId::new(
        eframe::egui::Order::Debug,
        eframe::egui::Id::new("performance-paint-complete"),
    ))
    .add(eframe::egui::PaintCallback {
        rect: ctx.content_rect(),
        callback: Arc::new(callback),
    });
}
