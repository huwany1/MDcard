// SPDX-License-Identifier: AGPL-3.0-only
//! Opt-in attribution of requested Rust heap bytes. Excludes C/native/GPU allocations
//! and allocator metadata. Headers retain ownership when values cross threads.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
const GROUPS: [&str; 6] = [
    "other-or-small-allocations",
    "markdown-parser",
    "reading-index",
    "reading-precision",
    "recovery",
    "document-save",
];
static LIVE: [AtomicUsize; 6] = [const { AtomicUsize::new(0) }; 6];
static PEAK: [AtomicUsize; 6] = [const { AtomicUsize::new(0) }; 6];
static TOTAL: AtomicUsize = AtomicUsize::new(0);
static TOTAL_PEAK: AtomicUsize = AtomicUsize::new(0);
thread_local! { static GROUP: Cell<usize> = const { Cell::new(usize::MAX) }; }
fn group(bytes: usize) -> usize {
    if bytes < 4096 {
        return 0;
    }
    GROUP
        .try_with(|slot| {
            if slot.get() != usize::MAX {
                return slot.get();
            }
            slot.set(0); // Thread handle initialization may itself allocate.
            let thread = std::thread::current();
            let id = match thread.name().unwrap_or("") {
                "mdcard-incremental-parser" => 1,
                "reading-index" => 2,
                "reading-precision" => 3,
                name if name.starts_with("recovery-") => 4,
                "document-save" => 5,
                _ => 0,
            };
            slot.set(id);
            id
        })
        .unwrap_or(0)
}
fn added(id: usize, bytes: usize) {
    let live = LIVE[id].fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK[id].fetch_max(live, Ordering::Relaxed);
    let total = TOTAL.fetch_add(bytes, Ordering::Relaxed) + bytes;
    TOTAL_PEAK.fetch_max(total, Ordering::Relaxed);
}
fn removed(id: usize, bytes: usize) {
    LIVE[id].fetch_sub(bytes, Ordering::Relaxed);
    TOTAL.fetch_sub(bytes, Ordering::Relaxed);
}
fn expanded(layout: Layout) -> Option<(Layout, usize)> {
    let offset = layout.align().max(2 * size_of::<usize>());
    Some((
        Layout::from_size_align(layout.size().checked_add(offset)?, offset).ok()?,
        offset,
    ))
}
struct Profiled;
#[global_allocator]
static ALLOCATOR: Profiled = Profiled;
// Safety: each returned allocation preserves the requested alignment, has a
// private prefix, and is returned to System using exactly its expanded Layout.
unsafe impl GlobalAlloc for Profiled {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some((full, offset)) = expanded(layout) else {
            return std::ptr::null_mut();
        };
        let base = unsafe { System.alloc(full) };
        if base.is_null() {
            return base;
        }
        let id = group(layout.size());
        let ptr = unsafe { base.add(offset) };
        unsafe {
            ptr.sub(2 * size_of::<usize>()).cast::<usize>().write(id);
        }
        added(id, layout.size());
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (full, offset) = expanded(layout).unwrap();
        let id = unsafe { ptr.sub(2 * size_of::<usize>()).cast::<usize>().read() };
        removed(id, layout.size());
        unsafe {
            System.dealloc(ptr.sub(offset), full);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let (full, offset) = expanded(layout).unwrap();
        let Some(total_size) = new_size.checked_add(offset) else {
            return std::ptr::null_mut();
        };
        let mut id = unsafe { ptr.sub(2 * size_of::<usize>()).cast::<usize>().read() };
        let next = unsafe { System.realloc(ptr.sub(offset), full, total_size) };
        if next.is_null() {
            return next;
        }
        if id == 0 && layout.size() < 4096 && new_size >= 4096 {
            removed(id, layout.size());
            id = group(new_size);
            added(id, new_size);
            unsafe {
                next.add(offset)
                    .sub(2 * size_of::<usize>())
                    .cast::<usize>()
                    .write(id);
            }
        } else if new_size >= layout.size() {
            added(id, new_size - layout.size());
        } else {
            removed(id, layout.size() - new_size);
        }
        unsafe { next.add(offset) }
    }
}
pub fn report() -> serde_json::Value {
    serde_json::json!({"tree_sitter_c_live_bytes":C_LIVE.load(Ordering::Relaxed),"tree_sitter_c_peak_bytes":C_PEAK.load(Ordering::Relaxed),"requested_live_bytes":TOTAL.load(Ordering::Relaxed),"requested_peak_bytes":TOTAL_PEAK.load(Ordering::Relaxed),"groups":GROUPS.iter().enumerate().map(|(i,name)|serde_json::json!({"name":name,"live_bytes":LIVE[i].load(Ordering::Relaxed),"peak_bytes":PEAK[i].load(Ordering::Relaxed)})).collect::<Vec<_>>(),"note":"Opt-in instrumentation changes allocation overhead. Rust requested bytes exclude native/GPU/allocator metadata; Tree-sitter core C requests are counted separately. Group 0 includes unnamed threads and allocations below 4096B; per-group peaks are not simultaneous."})
}

static C_LIVE: AtomicUsize = AtomicUsize::new(0);
static C_PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe extern "C" fn ts_malloc(size: usize) -> *mut std::ffi::c_void {
    let Some(total) = size.checked_add(16) else {
        return std::ptr::null_mut();
    };
    let Ok(layout) = Layout::from_size_align(total, 16) else {
        return std::ptr::null_mut();
    };
    let base = unsafe { System.alloc(layout) };
    if base.is_null() {
        return base.cast();
    }
    unsafe {
        base.cast::<usize>().write(size);
    }
    let live = C_LIVE.fetch_add(size, Ordering::Relaxed) + size;
    C_PEAK.fetch_max(live, Ordering::Relaxed);
    unsafe { base.add(16).cast() }
}
unsafe extern "C" fn ts_free(ptr: *mut std::ffi::c_void) {
    if ptr.is_null() {
        return;
    }
    let base = unsafe { ptr.cast::<u8>().sub(16) };
    let size = unsafe { base.cast::<usize>().read() };
    C_LIVE.fetch_sub(size, Ordering::Relaxed);
    unsafe {
        System.dealloc(base, Layout::from_size_align_unchecked(size + 16, 16));
    }
}
unsafe extern "C" fn ts_calloc(count: usize, size: usize) -> *mut std::ffi::c_void {
    let Some(size) = count.checked_mul(size) else {
        return std::ptr::null_mut();
    };
    let ptr = unsafe { ts_malloc(size) };
    if !ptr.is_null() {
        unsafe {
            ptr.cast::<u8>().write_bytes(0, size);
        }
    }
    ptr
}
unsafe extern "C" fn ts_realloc(ptr: *mut std::ffi::c_void, size: usize) -> *mut std::ffi::c_void {
    if ptr.is_null() {
        return unsafe { ts_malloc(size) };
    }
    if size == 0 {
        unsafe {
            ts_free(ptr);
        }
        return std::ptr::null_mut();
    }
    let Some(total) = size.checked_add(16) else {
        return std::ptr::null_mut();
    };
    let base = unsafe { ptr.cast::<u8>().sub(16) };
    let old_size = unsafe { base.cast::<usize>().read() };
    let next = unsafe {
        System.realloc(
            base,
            Layout::from_size_align_unchecked(old_size + 16, 16),
            total,
        )
    };
    if next.is_null() {
        return next.cast();
    }
    unsafe {
        next.cast::<usize>().write(size);
    }
    let live = if size >= old_size {
        C_LIVE.fetch_add(size - old_size, Ordering::Relaxed) + size - old_size
    } else {
        C_LIVE.fetch_sub(old_size - size, Ordering::Relaxed) - (old_size - size)
    };
    C_PEAK.fetch_max(live, Ordering::Relaxed);
    unsafe { next.add(16).cast() }
}
/// Called once at process entry, before any parser or parsing thread exists.
pub fn install_tree_sitter_allocator() {
    unsafe {
        tree_sitter::set_allocator(
            Some(ts_malloc),
            Some(ts_calloc),
            Some(ts_realloc),
            Some(ts_free),
        );
    }
}
