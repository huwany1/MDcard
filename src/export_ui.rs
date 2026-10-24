// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! The native UI only snapshots documents and polls events. All converters run off-thread.
use crate::{egui, localization::Language, theme::Palette};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const FORMATS: [(&str, &str, &str); 9] = [
    ("html", "Self-contained HTML", "html"),
    ("pdf", "PDF document", "pdf"),
    ("docx", "Word document", "docx"),
    ("png", "PNG long image", "png"),
    ("jpeg", "JPEG long image", "jpg"),
    ("epub", "EPUB ebook", "epub"),
    ("marp-html", "Marp HTML slides", "html"),
    ("marp-pdf", "Marp PDF slides", "pdf"),
    ("marp-pptx", "Marp PowerPoint slides", "pptx"),
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    format: usize,
    pipeline: String,
    theme: usize,
    paper: String,
    font_size: f32,
    margin: f32,
    scale: f32,
    line_numbers: bool,
    toc: bool,
    pandoc: String,
    reference: String,
    title: String,
    author: String,
    identifier: String,
    book_language: String,
    css: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            format: 1,
            pipeline: "native".into(),
            theme: 0,
            paper: "A4".into(),
            font_size: 16.,
            margin: 18.,
            scale: 2.,
            line_numbers: true,
            toc: true,
            pandoc: String::new(),
            reference: String::new(),
            title: String::new(),
            author: String::new(),
            identifier: String::new(),
            book_language: "en".into(),
            css: String::new(),
        }
    }
}
fn settings_path() -> PathBuf {
    crate::preferences::path().with_file_name("export-settings.json")
}
fn hex(c: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}
fn renderer_root() -> PathBuf {
    std::env::var_os("MDCARD_RENDERER_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("renderer")))
                .filter(|p| p.join("export.mjs").exists())
        })
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("renderer"))
}
#[cfg(any(not(target_os = "linux"), test))]
fn kill_tree(pid: u32) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let taskkill = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
            .join("System32/taskkill.exe");
        let status = std::process::Command::new(taskkill)
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .creation_flags(0x08000000)
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(
                "Export process-tree termination failed",
            ));
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        crate::process_tree::cancel(pid, Duration::from_secs(5))
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let status = std::process::Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(
                "Export process-group termination failed",
            ));
        }
        Ok(())
    }
}

// Never join a reader until EOF has been observed. A converter can exit while
// an undiscovered descendant still owns a pipe; cleanup success alone is not EOF.
fn confirm_export_streams<T, U>(
    reader: &std::thread::JoinHandle<T>,
    errors: &std::thread::JoinHandle<U>,
    timeout: Duration,
    cleanup: std::io::Result<()>,
    committed: &AtomicBool,
) -> std::io::Result<()> {
    let closing = Instant::now();
    while !reader.is_finished() || !errors.is_finished() {
        if closing.elapsed() >= timeout {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Converter streams did not close before the cleanup deadline; check the destination before retrying",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // A complete event can arrive after the root exits but before pipe readers
    // finish. Accept that authoritative result only after both readers finished.
    cleanup.or_else(|error| {
        if committed.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(error)
        }
    })
}

struct Task {
    thread: Option<std::thread::JoinHandle<()>>,
    rx: mpsc::Receiver<Value>,
    cancel: Arc<AtomicBool>,
    started: Instant,
    snapshot: u64,
    output: PathBuf,
}
impl Drop for Task {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub struct ExportUi {
    pub open: bool,
    pub secondary: bool,
    source_identity: Option<PathBuf>,
    source_generation: u64,
    settings: Settings,
    task: Option<Task>,
    last: Value,
    progress: f32,
    stage: String,
    error: Option<Value>,
    rects: serde_json::Map<String, Value>,
    snapshot: Option<u64>,
    output: Option<PathBuf>,
    report_at: Instant,
}
impl Default for ExportUi {
    fn default() -> Self {
        let settings = std::fs::read(settings_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .unwrap_or_default();
        Self {
            open: false,
            secondary: false,
            source_identity: None,
            source_generation: 0,
            settings,
            task: None,
            last: Value::Null,
            progress: 0.,
            stage: String::new(),
            error: None,
            rects: Default::default(),
            snapshot: None,
            output: None,
            report_at: Instant::now(),
        }
    }
}
impl ExportUi {
    pub fn open_for(&mut self, secondary: bool, path: Option<&Path>, generation: u64) {
        self.secondary = secondary;
        self.source_identity = path.map(Path::to_path_buf);
        self.source_generation = generation;
        self.open = true;
    }
    // The frozen document and pane identity travel together through this UI boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        language: Language,
        source: &str,
        source_path: Option<&Path>,
        generation: u64,
        protected: &[PathBuf],
        palette: Palette,
    ) {
        self.poll();
        if !self.open {
            self.report(source);
            return;
        }
        let mut open = self.open;
        let mut launch = false;
        self.rects.clear();
        let response=egui::Window::new(crate::theme::window_title(language.text("Export document")))
        .id(egui::Id::new("lossless-export"))
        .open(&mut open)
        .default_width(610.)
        .min_width(260.)
        .max_width(crate::theme::window_bounds(ctx).width()-16.)
        .constrain_to(crate::theme::window_bounds(ctx))
        .max_height((ctx.content_rect().height()-65.).max(240.))
        .resizable(true)
        .show(ctx,|ui|{
            let running=self.task.is_some();
            egui::ScrollArea::vertical()
            .id_salt("export-configuration")
            .max_height((ui.available_height()-110.).max(40.))
            .show(ui,|ui|{
                ui.label(format!("{}: {}",language.text("Source document"),source_path.map(|p|p.display().to_string()).unwrap_or_else(||language.text("Untitled").into())));
                ui.label(language.text("Export a snapshot of your current document. You can keep editing."));
                ui.add_enabled_ui(!running,|ui|{
                    let r=egui::ComboBox::from_id_salt("export-format")
                    .selected_text(language.text(FORMATS[self.settings.format.min(8)].1))
                    .show_ui(ui,|ui|{
                        for(i,(_,label,_)) in FORMATS.iter().enumerate(){
                            let r=ui.selectable_value(&mut self.settings.format,i,language.text(label));
                            self.rects.insert(format!("format-{i}"),rect(r.rect));
                        }
                    }
                    );
                    self.rects.insert("format".into(),rect(r.response.rect));
                    let kind=FORMATS[self.settings.format.min(8)].0;
                    if kind=="docx"||kind=="epub"{
                        self.settings.pipeline="pandoc".into();
                        ui.label(language.text("Pandoc pipeline"));
                    }
                    else{
                        self.settings.pipeline="native".into();
                        ui.label(language.text("Built-in pipeline · bundled runtimes"));
                    }
                    ui.horizontal_wrapped(|ui|{
                        ui.label(language.text("Theme"));
                        egui::ComboBox::from_id_salt("export-theme")
                        .selected_text(language.text(["Current editor theme","Light","Dark"][self.settings.theme.min(2)]))
                        .show_ui(ui,|ui|{
                            for(i,label)in["Current editor theme","Light","Dark"].iter().enumerate(){
                                ui.selectable_value(&mut self.settings.theme,i,language.text(label));
                            }
                        }
                        );
                        ui.label(language.text("Paper"));
                        egui::ComboBox::from_id_salt("export-paper")
                        .selected_text(&self.settings.paper)
                        .show_ui(ui,|ui|{
                            ui.selectable_value(&mut self.settings.paper,"A4".into(),"A4");
                            ui.selectable_value(&mut self.settings.paper,"Letter".into(),"Letter");
                        }
                        );
                    }
                    );
                    ui.horizontal_wrapped(|ui|{
                        ui.label(language.text("Font size"));
                        ui.add(egui::DragValue::new(&mut self.settings.font_size).range(10.0..=28.0));
                        ui.label(language.text("Margins (mm)"));
                        ui.add(egui::DragValue::new(&mut self.settings.margin).range(8.0..=40.0));
                        ui.label(language.text("Image scale"));
                        ui.add(egui::DragValue::new(&mut self.settings.scale).range(1.0..=4.0).speed(0.25));
                    }
                    );
                    ui.horizontal_wrapped(|ui|{
                        ui.checkbox(&mut self.settings.toc,language.text("Clickable table of contents"));
                        ui.checkbox(&mut self.settings.line_numbers,language.text("Code line numbers"));
                    }
                    );
                    ui.collapsing(language.text("Document metadata"),|ui|{
                        for(label,value)in[("Title",&mut self.settings.title),("Author",&mut self.settings.author),("Book identifier",&mut self.settings.identifier),("Book language",&mut self.settings.book_language)]{
                            ui.horizontal_wrapped(|ui|{
                                ui.label(language.text(label));
                                ui.text_edit_singleline(value);
                            }
                            );
                        }
                    }
                    );
                    ui.collapsing(language.text("Custom CSS"),|ui|{
                        ui.label(language.text("Inline CSS only. Imports and active content are rejected."));
                        let r=ui.add(egui::TextEdit::multiline(&mut self.settings.css).code_editor().desired_rows(4).desired_width(f32::INFINITY));
                        self.rects.insert("css".into(),rect(r.rect));
                    }
                    );
                    ui.collapsing(language.text("Pandoc configuration"),|ui|{
                        ui.label(language.text("Leave blank to use bundled Pandoc, then PATH. DOCX and EPUB require Pandoc."));
                        ui.horizontal_wrapped(|ui|{
                            ui.label(language.text("Pandoc executable"));
                            let r=ui.text_edit_singleline(&mut self.settings.pandoc);
                            self.rects.insert("pandoc".into(),rect(r.rect));
                            if ui.button(language.text("Browse…")).clicked() && let Some(p)=rfd::FileDialog::new().pick_file(){
                                self.settings.pandoc=p.display().to_string();
                            }
                        }
                        );
                        ui.horizontal_wrapped(|ui|{
                            ui.label(language.text("Reference DOCX"));
                            ui.text_edit_singleline(&mut self.settings.reference);
                            if ui.button(language.text("Browse…")).clicked() && let Some(p)=rfd::FileDialog::new().add_filter("Word",&["docx"]).pick_file(){
                                self.settings.reference=p.display().to_string();
                            }
                        }
                        );
                        ui.hyperlink_to(language.text("Download Pandoc"),"https://pandoc.org/installing.html");
                    }
                    );
                    let same_source=self.source_identity.as_deref()==source_path&&self.source_generation==generation;
                    if !same_source {
                        ui.label(language.text("Source document changed. Reopen Export for the current document."));
                    }
                }
                );
                if let Some(error)=&self.error{
                    let stage=error["stage"].as_str().unwrap_or("export");
                    ui.colored_label(ui.visuals().error_fg_color,language.text(if stage=="cleanup"||stage=="publication" {"Export result is unconfirmed. Your document is kept; check the destination before retrying."} else {"Export failed. Your document and previous output are preserved."}));
                    ui.label(format!("{}: {}",language.text("Stage"),language.text(stage_label(stage))));
                    if let Some(line)=error["line"].as_u64(){
                        ui.label(format!("{}: {line}",language.text("Source line")));
                    }
                    if let Some(resource)=error["resource"].as_str(){
                        ui.label(resource);
                    }
                    ui.label(error["message"].as_str().unwrap_or(""));
                    if stage=="dependency"{
                        ui.hyperlink_to(language.text("Download Pandoc"),"https://pandoc.org/installing.html");
                        ui.label(language.text("Reinstall the complete MDcard package if a bundled runtime is missing."));
                    }
                }
                else if self.last["type"]=="complete"{
                    ui.label(language.text("Export complete"));
                    ui.label(self.last["output"].as_str().unwrap_or(""));
                }
                else if self.last["type"]=="cancelled"{
                    ui.label(language.text("Export cancelled. Previous output preserved."));
                }
            }
            );
            ui.separator();
            // Task controls stay outside the scrolling configuration area at every window size.
            if running{
                let width=ui.available_width().max(60.);
                let r=ui.add(egui::ProgressBar::new(self.progress/100.).desired_width(width).show_percentage());
                self.rects.insert("progress".into(),rect(r.rect));
                ui.label(language.text(stage_label(&self.stage)));
                if self.last["type"]!="complete"{
                    let r=ui.button(language.text("Cancel export"));
                    if ui.clip_rect().contains_rect(r.rect){
                        self.rects.insert("cancel".into(),rect(r.rect));
                    }
                    if r.clicked(){
                        if let Some(task)=&self.task{
                            task.cancel.store(true,Ordering::Release);
                        }
                        self.stage="cancelling".into();
                    }
                }
            }
            else{
                let same_source=self.source_identity.as_deref()==source_path&&self.source_generation==generation;
                let r=ui.add_enabled(same_source,egui::Button::new(language.text("Export…")));
                if ui.clip_rect().contains_rect(r.rect){
                    self.rects.insert("export".into(),rect(r.rect));
                }
                if r.clicked(){
                    launch=true;
                }
            }
        }
        );
        if response
            .as_ref()
            .is_some_and(|r| crate::theme::window_escape(ctx, r.response.layer_id))
        {
            open = false;
        }
        self.open = open;
        if launch {
            let format = FORMATS[self.settings.format.min(8)];
            let name = source_path
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("document");
            if let Some(output) = rfd::FileDialog::new()
                .add_filter(language.text(format.1), &[format.2])
                .set_file_name(format!("{name}.{}", format.2))
                .set_directory(source_path.and_then(Path::parent).unwrap_or(Path::new(".")))
                .save_file()
            {
                if protected.iter().any(|p| same_path(p, &output))
                    || source_path.is_some_and(|p| same_path(p, &output))
                {
                    self.error = Some(
                        json!({"stage":"request","message":language.text("Choose a different destination. An open document cannot be overwritten by export.")}),
                    );
                } else {
                    self.start(ctx, source, source_path, output, palette);
                }
            }
        }
        if self.task.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        self.report(source);
    }
    fn start(
        &mut self,
        ctx: &egui::Context,
        source: &str,
        source_path: Option<&Path>,
        output: PathBuf,
        palette: Palette,
    ) {
        let root = renderer_root();
        let s = &self.settings;
        let dark = match s.theme {
            1 => false,
            2 => true,
            _ => palette.dark,
        };
        let p = if s.theme == 0 {
            palette
        } else {
            Palette::new(dark, 0)
        };
        let bundled_pandoc = root.join("runtime/pandoc").join(if cfg!(windows) {
            "pandoc.exe"
        } else {
            "pandoc"
        });
        let dev_pandoc = root
            .join("runtime")
            .join(if cfg!(windows) { "windows" } else { "linux" })
            .join("pandoc")
            .join(if cfg!(windows) {
                "pandoc.exe"
            } else {
                "pandoc"
            });
        let pandoc = if !s.pandoc.is_empty() {
            s.pandoc.clone()
        } else if bundled_pandoc.exists() {
            bundled_pandoc.display().to_string()
        } else if dev_pandoc.exists() {
            dev_pandoc.display().to_string()
        } else {
            "pandoc".into()
        };
        let base = source_path
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let request = json!({"source":source,"sourcePath":source_path,"output":output,"format":FORMATS[s.format.min(8)].0,"baseDir":base,"options":{"pipeline":s.pipeline,"dark":dark,"background":hex(p.paper),"foreground":hex(p.ink),"accent":hex(p.accent),"fontSize":s.font_size,"marginMm":s.margin,"scale":s.scale,"paper":s.paper,"lineNumbers":s.line_numbers,"toc":s.toc,"pandocPath":pandoc,"referenceDocx":s.reference,"title":s.title,"author":s.author,"identifier":s.identifier,"language":s.book_language,"customCss":s.css}});
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelling = cancel.clone();
        let repaint = ctx.clone();
        let target = output.clone();
        self.snapshot = Some(crate::rich_nodes::hash(source));
        self.output = Some(output.clone());
        self.last = Value::Null;
        self.error = None;
        self.stage = "parse".into();
        self.progress = 0.;
        self.task = Some(Task {
            thread: None,
            rx,
            cancel,
            started: Instant::now(),
            snapshot: self.snapshot.unwrap(),
            output,
        });
        let saved = serde_json::to_vec_pretty(s).unwrap_or_default();
        let settings = settings_path();
        let thread = std::thread::spawn(move || {
            let send = |value: Value| {
                let _ = tx.send(value);
                repaint.request_repaint();
            };
            let result = (|| -> Result<(), String> {
                if let Some(parent) = settings.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    let mut temp =
                        tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
                    temp.write_all(&saved).map_err(|e| e.to_string())?;
                    temp.persist(&settings).map_err(|e| e.to_string())?;
                }
                let parent = target.parent().ok_or("Missing output directory")?;
                let workspace = tempfile::Builder::new()
                    .prefix(".mdcard-task-")
                    .tempdir_in(parent)
                    .map_err(|e| e.to_string())?;
                let mut request = request;
                request["workDir"] = json!(workspace.path().join("render"));
                let input = workspace.path().join("request.json");
                std::fs::write(&input, serde_json::to_vec(&request).unwrap())
                    .map_err(|e| e.to_string())?;
                let node =
                    root.join("runtime")
                        .join(if cfg!(windows) { "node.exe" } else { "node" });
                if !node.exists() {
                    return Err(
                        "Bundled Node is missing. Reinstall the complete MDcard package.".into(),
                    );
                }
                let mut command = std::process::Command::new(node);
                #[cfg(target_os = "linux")]
                crate::process_tree::ensure_available().map_err(|e|format!("Reliable export cancellation is unavailable (Linux pidfd support is required): {e}"))?;
                command
                    .arg(root.join("export.mjs"))
                    .arg(input)
                    .current_dir(&root)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    command.process_group(0);
                }
                let mut child = command.spawn().map_err(|e| e.to_string())?;
                let id = child.id();
                #[cfg(target_os = "linux")]
                let task_group = match crate::process_tree::OwnedGroup::capture(id) {
                    Ok(group) => group,
                    Err(error) => {
                        // A spawn succeeded: Child::drop neither terminates nor reaps.
                        let cleanup = child.kill();
                        let reaped = child.wait();
                        send(
                            json!({"type":"error","stage":"cleanup","message":format!("Export ownership could not be captured: {error}; direct termination: {cleanup:?}; direct wait: {reaped:?}. Descendant cleanup is unconfirmed; check the destination before retrying.")}),
                        );
                        return Ok(());
                    }
                };
                let terminate_tree = || {
                    #[cfg(target_os = "linux")]
                    {
                        task_group.cancel(Duration::from_secs(5))
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        kill_tree(id)
                    }
                };
                let stdout = child.stdout.take().unwrap();
                let stderr = child.stderr.take().unwrap();
                let events = tx.clone();
                let paint = repaint.clone();
                let committed = Arc::new(AtomicBool::new(false));
                let publication = committed.clone();
                let reader = std::thread::spawn(move || {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        if let Ok(event) = serde_json::from_str::<Value>(&line) {
                            if event["type"] == "complete" {
                                publication.store(true, Ordering::Release);
                            }
                            let _ = events.send(event);
                            paint.request_repaint();
                        }
                    }
                });
                let errors = std::thread::spawn(move || {
                    let mut result = String::new();
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        if result.len() < 8000 {
                            result.push_str(&line);
                            result.push('\n');
                        }
                    }
                    result
                });
                let started = Instant::now();
                let mut aborted = false;
                loop {
                    if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                        if !reader.is_finished() || !errors.is_finished() {
                            let cleanup = terminate_tree();
                            if let Err(error) = confirm_export_streams(
                                &reader,
                                &errors,
                                Duration::from_secs(1),
                                cleanup,
                                &committed,
                            ) {
                                send(
                                    json!({"type":"error","stage":"cleanup","message":format!("The converter exited, but cleanup was not confirmed: {error}")}),
                                );
                                return Ok(());
                            }
                        }
                        break;
                    }
                    if cancelling.load(Ordering::Acquire)
                        || started.elapsed() > Duration::from_secs(190)
                    {
                        aborted = !committed.load(Ordering::Acquire);
                        let cleanup = terminate_tree();
                        let direct_kill = child.kill();
                        if direct_kill.is_err()
                            && child.try_wait().map_err(|e| e.to_string())?.is_none()
                        {
                            send(
                                json!({"type":"error","stage":"cleanup","message":"Export cleanup could not terminate the root process; cancellation is not confirmed."}),
                            );
                            return Ok(());
                        }
                        child.wait().map_err(|e| e.to_string())?;
                        if let Err(error) = cleanup {
                            if let Err(error) = confirm_export_streams(
                                &reader,
                                &errors,
                                Duration::from_secs(1),
                                Err(error),
                                &committed,
                            ) {
                                send(
                                    json!({"type":"error","stage":"cleanup","message":format!("Export cleanup was not confirmed: {error}")}),
                                );
                                return Ok(());
                            }
                            aborted = false;
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(40));
                }
                if let Err(error) = confirm_export_streams(
                    &reader,
                    &errors,
                    Duration::from_secs(1),
                    Ok(()),
                    &committed,
                ) {
                    send(json!({"type":"error","stage":"cleanup","message":error.to_string()}));
                    return Ok(());
                }
                let _ = reader.join();
                let stderr = errors.join().unwrap_or_default();
                aborted &= !committed.load(Ordering::Acquire);
                if aborted {
                    if workspace.path().join("publication-started").exists() {
                        send(
                            json!({"type":"error","stage":"publication","message":"Publication had started when cancellation was requested. The destination may contain the completed export; check it before retrying."}),
                        );
                    } else if cancelling.load(Ordering::Acquire) {
                        send(json!({"type":"cancelled","elapsedMs":started.elapsed().as_millis()}));
                    } else {
                        send(
                            json!({"type":"error","stage":"timeout","message":"Export exceeded its task deadline.","elapsedMs":started.elapsed().as_millis()}),
                        );
                    }
                } else if !stderr.is_empty() {
                    send(json!({"type":"diagnostic","message":stderr}));
                }
                // Workspace is owned by Rust and survives converter termination long enough to clean up.
                Ok(())
            })();
            if let Err(message) = result {
                send(json!({"type":"error","stage":"dependency","message":message}));
            }
            send(json!({"type":"finished"}));
        });
        self.task.as_mut().unwrap().thread = Some(thread);
    }
    fn poll(&mut self) {
        let events = self
            .task
            .as_ref()
            .map(|t| t.rx.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event["type"].as_str().unwrap_or("") {
                "progress" => {
                    self.progress = event["progress"].as_f64().unwrap_or(0.) as f32;
                    self.stage = event["stage"].as_str().unwrap_or("").into();
                }
                "error" => {
                    self.error = Some(event.clone());
                    self.last = event;
                }
                "complete" | "cancelled" => {
                    self.last = event;
                }
                "finished" => {
                    self.task = None;
                    if self.last.is_null() && self.error.is_none() {
                        self.error = Some(
                            json!({"stage":"export","message":"Export process ended without a complete result"}),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    fn report(&mut self, source: &str) {
        if self.report_at.elapsed() < Duration::from_millis(150) {
            return;
        }
        self.report_at = Instant::now();
        if let Some(path) = std::env::var_os("MDCARD_EXPORT_REPORT") {
            let result = json!({"open":self.open,"running":self.task.is_some(),"stage":self.stage,"progress":self.progress,"error":self.error,"last":self.last,"snapshotHash":self.snapshot.map(|h|h.to_string()),"currentSourceHash":crate::rich_nodes::hash(source).to_string(),"sourceUnchanged":self.snapshot==Some(crate::rich_nodes::hash(source)),"output":self.output,"controls":self.rects,"task":self.task.as_ref().map(|t|json!({"elapsedMs":t.started.elapsed().as_millis(),"snapshotHash":t.snapshot.to_string(),"output":t.output}))});
            let _ = std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap_or_default());
        }
    }
}
fn rect(r: egui::Rect) -> Value {
    json!({"x":r.min.x,"y":r.min.y,"w":r.width(),"h":r.height()})
}
fn stage_label(s: &str) -> &'static str {
    match s {
        "parse" => "Parsing Markdown…",
        "compile" => "Compiling formulas and diagrams…",
        "resources" => "Embedding document resources…",
        "layout" => "Laying out pages…",
        "write" => "Writing output…",
        "capture" => "Capturing image tiles…",
        "verify" => "Verifying output…",
        "complete" => "Export complete",
        "cancelling" => "Cancelling export…",
        "dependency" => "Converter configuration",
        "resource" | "preprocess" | "style" => "Document resources",
        "pagination" => "Print pagination",
        "math" | "diagram" => "Formulas and diagrams",
        _ => "Export document",
    }
}
fn same_path(a: &Path, b: &Path) -> bool {
    let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let a = canonical(a);
    let b = canonical(b);
    if cfg!(windows) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    #[test]
    fn successful_cleanup_without_eof_has_a_deadline_even_after_complete() {
        let (release, held) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            held.recv().unwrap();
        });
        let errors = std::thread::spawn(String::new);
        let committed = AtomicBool::new(true);
        let result = confirm_export_streams(
            &reader,
            &errors,
            Duration::from_millis(20),
            Ok(()),
            &committed,
        );
        release.send(()).unwrap();
        reader.join().unwrap();
        errors.join().unwrap();
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    }
    #[test]
    fn complete_delivered_after_root_exit_wins_only_after_streams_close() {
        let committed = Arc::new(AtomicBool::new(false));
        let published = committed.clone();
        let (release, pending) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            pending.recv().unwrap();
            published.store(true, Ordering::Release);
        });
        let errors = std::thread::spawn(String::new);
        let cleanup = Err(std::io::Error::other("root already exited"));
        assert!(!committed.load(Ordering::Acquire));
        release.send(()).unwrap();
        confirm_export_streams(
            &reader,
            &errors,
            Duration::from_secs(1),
            cleanup,
            &committed,
        )
        .unwrap();
        reader.join().unwrap();
        errors.join().unwrap();
    }
    #[test]
    fn cleanup_error_without_complete_is_not_suppressed_by_closed_streams() {
        let reader = std::thread::spawn(|| ());
        let errors = std::thread::spawn(String::new);
        let committed = AtomicBool::new(false);
        let result = confirm_export_streams(
            &reader,
            &errors,
            Duration::from_secs(1),
            Err(std::io::Error::other("cleanup failed")),
            &committed,
        );
        reader.join().unwrap();
        errors.join().unwrap();
        assert!(result.unwrap_err().to_string().contains("cleanup failed"));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn reparented_descendant_holding_stdout_cannot_block_cleanup_handoff() {
        use std::io::Read;
        use std::process::{Command, Stdio};
        // The independent supervisor reaps the converter root before announcing
        // readiness. Only its adopted cat descendant keeps stdout open; stdin
        // is a control pipe for deterministic fixture cleanup after the assertion.
        let script = r#"
import ctypes,subprocess,sys,os
assert ctypes.CDLL(None).prctl(36,1,0,0,0)==0
r,w=os.pipe()
code="import subprocess,sys; p=subprocess.Popen(['/bin/cat'],stdin=int(sys.argv[1])); print(p.pid,file=sys.stderr,flush=True)"
root=subprocess.Popen([sys.executable,'-c',code,str(r)],pass_fds=(r,))
assert root.wait()==0
os.close(r)
os.close(1)
print('ROOT_REAPED',file=sys.stderr,flush=True)
sys.stdin.readline()
os.close(w)
waited,status=os.waitpid(-1,0)
assert os.WIFEXITED(status) and os.WEXITSTATUS(status)==0
"#;
        let mut supervisor = Command::new("/usr/bin/python3")
            .args(["-u", "-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stderr = BufReader::new(supervisor.stderr.take().unwrap());
        let mut line = String::new();
        stderr.read_line(&mut line).unwrap();
        assert!(line.trim().parse::<u32>().is_ok());
        line.clear();
        stderr.read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "ROOT_REAPED");
        let stdout = supervisor.stdout.take().unwrap();
        let reader = std::thread::spawn(move || {
            let mut output = String::new();
            BufReader::new(stdout).read_to_string(&mut output).unwrap();
        });
        let errors = std::thread::spawn(move || {
            let mut output = String::new();
            stderr.read_to_string(&mut output).unwrap();
            output
        });
        let committed = AtomicBool::new(false);
        let result = confirm_export_streams(
            &reader,
            &errors,
            Duration::from_millis(30),
            Ok(()),
            &committed,
        );
        supervisor
            .stdin
            .take()
            .unwrap()
            .write_all(b"release\n")
            .unwrap();
        assert!(supervisor.wait().unwrap().success());
        reader.join().unwrap();
        errors.join().unwrap();
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod process_tests {
    use super::*;
    #[test]
    #[ignore = "explicit cancellation handoff probe under CPU contention"]
    fn followup_cancellation_handoff_records_live_vs_terminal_descendants() {
        use std::os::unix::process::CommandExt;
        unsafe extern "C" {
            fn prctl(option: i32, ...) -> i32;
            fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        }
        // Test-local subreaper: known detached fixture children can be reaped
        // after cancellation rather than leaving zombies with container PID 1.
        assert_eq!(unsafe { prctl(36, 1, 0, 0, 0) }, 0);
        let stop = Arc::new(AtomicBool::new(false));
        let loads: Vec<_> = (0..4)
            .map(|_| {
                let stop = stop.clone();
                std::thread::spawn(move || {
                    let mut x = 1u64;
                    while !stop.load(Ordering::Relaxed) {
                        for _ in 0..1000 {
                            x = std::hint::black_box(
                                x.wrapping_mul(6364136223846793005).wrapping_add(1),
                            );
                        }
                    }
                })
            })
            .collect();
        let mut unrelated = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let terminal = |s: &str| {
            s.is_empty()
                || s.lines()
                    .any(|l| l.starts_with("State:") && l.contains('Z'))
        };
        let mut observations = Vec::new();
        let mut live_returns = 0;
        let mut still_live = 0;
        for iteration in 0..120 {
            let mut parent = std::process::Command::new("/bin/sh")
                .args(["-c", "/usr/bin/setsid /bin/sleep 30 & echo $!; wait"])
                .process_group(0)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let mut line = String::new();
            BufReader::new(parent.stdout.take().unwrap())
                .read_line(&mut line)
                .unwrap();
            let descendant: u32 = line.trim().parse().unwrap();
            let start = Instant::now();
            kill_tree(parent.id()).unwrap();
            let returned_us = start.elapsed().as_micros();
            let initial =
                std::fs::read_to_string(format!("/proc/{descendant}/status")).unwrap_or_default();
            if !terminal(&initial) {
                live_returns += 1;
            }
            parent.wait().unwrap();
            let mut observed = initial.clone();
            while !terminal(&observed) && start.elapsed() < Duration::from_secs(2) {
                std::thread::yield_now();
                observed = std::fs::read_to_string(format!("/proc/{descendant}/status"))
                    .unwrap_or_default();
            }
            if !terminal(&observed) {
                still_live += 1;
                let _ = std::process::Command::new("/bin/kill")
                    .args(["-KILL", &descendant.to_string()])
                    .status();
            }
            let mut wait_status = 0;
            let reaped = unsafe { waitpid(descendant as i32, &mut wait_status, 0) };
            observations.push(json!({"iteration":iteration,"pid":descendant,"return_us":returned_us,"terminal_us":start.elapsed().as_micros(),"initial":initial,"final":observed,"reaped":reaped==descendant as i32,"wait_status":wait_status}));
        }
        stop.store(true, Ordering::Relaxed);
        for t in loads {
            t.join().unwrap();
        }
        let unrelated_preserved = unrelated.try_wait().unwrap().is_none();
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
        assert_eq!(unsafe { prctl(36, 0, 0, 0, 0) }, 0);
        let report = json!({"iterations":observations.len(),"live_at_return":live_returns,"live_after_2s":still_live,"unrelated_preserved":unrelated_preserved,"observations":observations});
        if let Ok(path) = std::env::var("MDCARD_FOLLOWUP_CANCEL_REPORT") {
            std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
        println!("FOLLOWUP_CANCEL={report}");
        assert!(unrelated_preserved);
        assert_eq!(
            live_returns, 0,
            "cancellation returned while an owned descendant was still live"
        );
        assert_eq!(still_live, 0);
    }
    #[test]
    fn cancellation_reaps_detached_descendants_and_keeps_unrelated_processes() {
        use std::os::unix::process::CommandExt;
        let mut unrelated = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let mut parent = std::process::Command::new("/bin/sh")
            .args(["-c", "/usr/bin/setsid /bin/sleep 30 & echo $!; wait"])
            .process_group(0)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(parent.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let descendant = line.trim().parse::<u32>().unwrap();
        kill_tree(parent.id()).unwrap();
        let status = parent.wait().unwrap();
        assert!(!status.success());
        let status =
            std::fs::read_to_string(format!("/proc/{descendant}/status")).unwrap_or_default();
        assert!(
            status.is_empty()
                || status
                    .lines()
                    .any(|line| line.starts_with("State:") && line.contains('Z')),
            "Detached export child must not remain running"
        );
        assert!(
            unrelated.try_wait().unwrap().is_none(),
            "Cancellation must not terminate unrelated processes"
        );
        let _ = unrelated.kill();
        let _ = unrelated.wait();
    }
}
