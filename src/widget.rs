use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::api::{self, UsageResponse};
use crate::config::Config;
use crate::cookies::{self, BrowserKind};

const BG: egui::Color32 = egui::Color32::from_rgb(0xf5, 0xf5, 0xf0);
const FG: egui::Color32 = egui::Color32::from_rgb(0x1a, 0x1a, 0x1a);
const DIM: egui::Color32 = egui::Color32::from_rgb(0x88, 0x88, 0x88);
const BAR_BG: egui::Color32 = egui::Color32::from_rgb(0xd9, 0xd9, 0xd9);
const BAR_BLUE: egui::Color32 = egui::Color32::from_rgb(0x4a, 0x90, 0xd9);
const BAR_YELLOW: egui::Color32 = egui::Color32::from_rgb(0xd4, 0xa0, 0x17);
const BAR_RED: egui::Color32 = egui::Color32::from_rgb(0xdc, 0x45, 0x45);
const FOOTER_DIM: egui::Color32 = egui::Color32::from_rgb(0xaa, 0xaa, 0xaa);

const BAR_W: f32 = 124.0;
const BAR_H: f32 = 10.0;
const TICK: Duration = Duration::from_secs(30);
const DEFAULT_REFRESH_SECS: u64 = 300;
const IDLE_THRESHOLD_SECS: u64 = 60;
const PADDING: f32 = 10.0;
const MIN_HEIGHT: f32 = 274.0;
const SNAP_SECS: f64 = 0.7;
const SNAP_STAGGER: f64 = 0.12;

const REFRESH_OPTIONS: &[(u64, &str)] = &[
    (60, "1 min"),
    (120, "2 min"),
    (300, "5 min"),
    (600, "10 min"),
    (900, "15 min"),
    (1800, "30 min"),
];

const WEEKLY_KEYS: &[(&str, &str)] = &[
    ("seven_day", "All models"),
    ("seven_day_opus", "Opus"),
    ("seven_day_sonnet", "Sonnet"),
    ("seven_day_cowork", "Cowork"),
];

fn ease_out_back(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let c1: f64 = 2.5;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn with_alpha(c: egui::Color32, a: f32) -> egui::Color32 {
    let [r, g, b, _] = c.to_array();
    egui::Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn bar_color(pct: f64) -> egui::Color32 {
    if pct < 75.0 {
        BAR_BLUE
    } else if pct < 90.0 {
        BAR_YELLOW
    } else {
        BAR_RED
    }
}

fn time_left(resets_at: Option<&str>) -> String {
    let Some(s) = resets_at else {
        return String::new();
    };
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) else {
        return String::new();
    };
    let now = chrono::Utc::now();
    let secs = (dt.with_timezone(&chrono::Utc) - now).num_seconds();
    if secs <= 0 {
        return "Resetting...".into();
    }
    let m = (secs / 60) % 60;
    let h = secs / 3600;
    if h >= 24 {
        format!("Resets in {}d {}h", h / 24, h % 24)
    } else if h > 0 {
        format!("Resets in {h} hr {m} min")
    } else {
        format!("Resets in {m} min")
    }
}

fn updated_ago(last: Instant) -> String {
    let ago = last.elapsed().as_secs();
    if ago < 60 {
        "Last updated: just now".into()
    } else if ago < 3600 {
        let m = ago / 60;
        let s = if m != 1 { "s" } else { "" };
        format!("Last updated: {m} minute{s} ago")
    } else {
        let h = ago / 3600;
        let s = if h != 1 { "s" } else { "" };
        format!("Last updated: {h} hour{s} ago")
    }
}

struct SharedState {
    data: Option<Result<UsageResponse, String>>,
    fetching: bool,
    account_name: Option<String>,
}

pub struct UsageApp {
    browser: Option<BrowserKind>,
    data_dir: Option<String>,
    picker_options: Option<Vec<BrowserKind>>,
    shared: Arc<Mutex<SharedState>>,
    cached_data: Option<Result<UsageResponse, String>>,
    last_fetch: Option<Instant>,
    last_fetch_start: Option<Instant>,
    refresh_secs: u64,
    title: String,
    title_explicit: bool,
    first_frame: bool,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    wm_name: String,
    always_on_top: bool,
    all_workspaces: bool,
    last_height: f32,
    data_arrived_at: Option<Instant>,
}

impl UsageApp {
    pub fn new(
        browser: Option<BrowserKind>,
        data_dir: Option<String>,
        picker_options: Option<Vec<BrowserKind>>,
        title: String,
        title_explicit: bool,
        wm_name: String,
        config: Config,
    ) -> Self {
        let refresh_secs = config.refresh_secs.unwrap_or(DEFAULT_REFRESH_SECS);
        let always_on_top = config.always_on_top.unwrap_or(true);
        let all_workspaces = config.all_workspaces.unwrap_or(true);
        Self {
            browser,
            data_dir,
            picker_options,
            shared: Arc::new(Mutex::new(SharedState {
                data: None,
                fetching: false,
                account_name: None,
            })),
            cached_data: None,
            last_fetch: None,
            last_fetch_start: None,
            refresh_secs,
            title,
            title_explicit,
            first_frame: true,
            wm_name,
            always_on_top,
            all_workspaces,
            last_height: 0.0,
            data_arrived_at: None,
        }
    }

    fn save_config(&self) {
        Config {
            refresh_secs: Some(self.refresh_secs),
            always_on_top: Some(self.always_on_top),
            all_workspaces: Some(self.all_workspaces),
        }
        .save();
    }

    fn start_fetch(&mut self) {
        let Some(browser) = self.browser else {
            return;
        };
        {
            let mut s = self.shared.lock().unwrap();
            if s.fetching {
                return;
            }
            s.fetching = true;
        }
        self.last_fetch_start = Some(Instant::now());
        let shared = Arc::clone(&self.shared);
        let data_dir = self.data_dir.clone();
        let need_name = !self.title_explicit && self.shared.lock().unwrap().account_name.is_none();
        std::thread::spawn(move || {
            let jar = cookies::read_cookies(browser, "claude.ai", data_dir.as_deref());
            let (result, name) = match jar {
                Ok(cookies) => {
                    let result = api::fetch_with_cookies(&cookies);
                    let name = if need_name {
                        api::fetch_account_name(&cookies).ok()
                    } else {
                        None
                    };
                    (result, name)
                }
                Err(e) => (Err(format!("Cookie error: {e}")), None),
            };
            let mut s = shared.lock().unwrap();
            s.data = Some(result);
            if let Some(n) = name {
                s.account_name = Some(n);
            }
            s.fetching = false;
        });
    }

    fn poll_result(&mut self) {
        let mut s = self.shared.lock().unwrap();
        if let Some(name) = s.account_name.take() {
            if !self.title_explicit {
                self.title = name;
            }
        }
        if let Some(result) = s.data.take() {
            match &result {
                Ok(_) => {
                    let first_success = !matches!(&self.cached_data, Some(Ok(_)));
                    self.cached_data = Some(result);
                    self.last_fetch = Some(Instant::now());
                    if first_success {
                        self.data_arrived_at = Some(Instant::now());
                    }
                }
                Err(_) => {
                    if self.cached_data.is_none() || self.cached_data.as_ref().is_some_and(|r| r.is_err()) {
                        self.cached_data = Some(result);
                    }
                }
            }
        }
    }

    fn should_refresh(&self) -> bool {
        if let Some(idle) = crate::idle::system_idle_secs() {
            if idle > IDLE_THRESHOLD_SECS {
                return false;
            }
        }
        match self.last_fetch_start {
            Some(t) => t.elapsed() >= Duration::from_secs(self.refresh_secs),
            None => true,
        }
    }
}

impl eframe::App for UsageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.first_frame {
            self.first_frame = false;
            ctx.style_mut(|s| {
                s.interaction.selectable_labels = false;
                s.visuals.panel_fill = BG;
                // Dark background for the right-click context menu.
                let menu_bg = egui::Color32::from_rgb(0x2a, 0x2a, 0x2a);
                s.visuals.widgets.noninteractive.bg_fill = menu_bg;
                s.visuals.window_fill = menu_bg;
                s.visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0x50, 0x50, 0x50));
            });
            let level = if self.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
        }

        // ESC to close
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Poll for results from background fetch
        self.poll_result();

        // Auto-refresh
        if self.browser.is_some() && self.picker_options.is_none() && self.should_refresh() {
            self.start_fetch();
        }

        // Schedule repaint for countdown updates
        ctx.request_repaint_after(TICK);

        egui::CentralPanel::default().show(ctx, |ui| {
                let panel_rect = ui.max_rect();

                // Drag interaction on whole panel
                let bg_response =
                    ui.interact(panel_rect, ui.id().with("drag"), egui::Sense::drag());

                // Close button in top-right
                let close_size = 22.0;
                let close_rect = egui::Rect::from_min_size(
                    egui::pos2(panel_rect.right() - close_size - 2.0, panel_rect.top() + 2.0),
                    egui::vec2(close_size, close_size),
                );
                let close_resp = ui.interact(close_rect, ui.id().with("close"), egui::Sense::click());
                let close_color = if close_resp.hovered() { FG } else { DIM };
                ui.painter().text(
                    close_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "\u{00d7}",
                    egui::FontId::proportional(16.0),
                    close_color,
                );
                if close_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                if bg_response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                bg_response.context_menu(|ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("Refresh interval").strong().size(11.0).color(egui::Color32::WHITE));
                        ui.separator();
                        for &(secs, label) in REFRESH_OPTIONS {
                            let current = self.refresh_secs == secs;
                            let text = if current {
                                egui::RichText::new(label)
                                    .size(13.0)
                                    .color(egui::Color32::WHITE)
                                    .font(egui::FontId::new(13.0, egui::FontFamily::Name("bold".into())))
                            } else {
                                egui::RichText::new(label)
                                    .size(11.0)
                                    .color(egui::Color32::WHITE)
                            };
                            let resp = ui.add(
                                egui::Label::new(text).sense(egui::Sense::click()),
                            );
                            if resp.clicked() {
                                self.refresh_secs = secs;
                                self.save_config();
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        {
                            let prefix = if self.always_on_top { "[x] " } else { "[ ] " };
                            let text = egui::RichText::new(format!("{prefix}Always on top"))
                                .size(11.0)
                                .color(egui::Color32::WHITE);
                            let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            if resp.clicked() {
                                self.always_on_top = !self.always_on_top;
                                let level = if self.always_on_top {
                                    egui::WindowLevel::AlwaysOnTop
                                } else {
                                    egui::WindowLevel::Normal
                                };
                                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                                self.save_config();
                                ui.close_menu();
                            }
                        }
                        {
                            let prefix = if self.all_workspaces { "[x] " } else { "[ ] " };
                            let text = egui::RichText::new(format!("{prefix}All workspaces"))
                                .size(11.0)
                                .color(egui::Color32::WHITE);
                            let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                            if resp.clicked() {
                                self.all_workspaces = !self.all_workspaces;
                                #[cfg(target_os = "linux")]
                                toggle_all_workspaces(self.wm_name.clone(), self.all_workspaces);
                                self.save_config();
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        let link = ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.label(egui::RichText::new("by: ").size(11.0).color(egui::Color32::WHITE));
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new("SmartAppsCo")
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(0x6a, 0xb0, 0xf0)),
                                )
                                .sense(egui::Sense::click()),
                            ).on_hover_cursor(egui::CursorIcon::PointingHand)
                        });
                        if link.inner.clicked() {
                            let _ = open::that("https://smartapps.co/from/claude-usage-widget");
                            ui.close_menu();
                        }
                    });
                });

                let inner_rect = egui::Rect::from_min_max(
                    panel_rect.min + egui::vec2(PADDING - 5.0, 0.0),
                    panel_rect.max - egui::vec2(PADDING, PADDING),
                );

                let content = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
                    // Header
                    if !self.title.is_empty() {
                        ui.label(
                            egui::RichText::new(&self.title)
                                .color(FG)
                                .size(16.0)
                                .font(egui::FontId::new(16.0, egui::FontFamily::Name("bold".into()))),
                        );
                        ui.add_space(4.0);
                    }

                    // Picker mode
                    if let Some(ref options) = self.picker_options.clone() {
                        ui.label(
                            egui::RichText::new("Session found in multiple browsers.")
                                .color(DIM)
                                .size(12.0),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Which browser to use?")
                                .color(FG)
                                .size(12.0),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            for &b in options {
                                let label = match b {
                                    BrowserKind::Firefox => "Firefox",
                                    BrowserKind::Chrome => "Chrome",
                                    BrowserKind::Brave => "Brave",
                                    BrowserKind::Edge => "Edge",
                                };
                                if ui.button(label).clicked() {
                                    self.browser = Some(b);
                                    self.picker_options = None;
                                    self.start_fetch();
                                }
                            }
                        });
                        return;
                    }

                    // Usage display
                    match &self.cached_data {
                        None => {
                            Self::render_loading(ui, ctx);
                        }
                        Some(Err(e)) => {
                            ui.label(
                                egui::RichText::new(e.as_str())
                                    .color(BAR_RED)
                                    .size(12.0),
                            );
                        }
                        Some(Ok(data)) => {
                            let elapsed = self.data_arrived_at.map_or(f64::MAX, |t| t.elapsed().as_secs_f64());
                            let mut section_idx: usize = 0;

                            // Five-hour (current session)
                            if let Some(bucket) = data.get("five_hour") {
                                let t = ((elapsed - section_idx as f64 * SNAP_STAGGER) / SNAP_SECS).clamp(0.0, 1.0);
                                Self::render_section(ui, "Current session", bucket.utilization.unwrap_or(0.0), bucket.resets_at.as_deref(), t);
                                section_idx += 1;
                            }

                            // Weekly limits
                            let weekly: Vec<_> = WEEKLY_KEYS
                                .iter()
                                .filter_map(|(k, label)| data.get(*k).map(|b| (*label, b)))
                                .collect();
                            if !weekly.is_empty() {
                                let header_t = ((elapsed - section_idx as f64 * SNAP_STAGGER) / SNAP_SECS).clamp(0.0, 1.0);
                                let header_alpha = (header_t * 6.0).min(1.0) as f32;
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new("Weekly limits")
                                        .color(with_alpha(FG, header_alpha))
                                        .size(16.0)
                                        .font(egui::FontId::new(16.0, egui::FontFamily::Name("bold".into()))),
                                );
                                ui.add_space(2.0);
                                for (label, bucket) in &weekly {
                                    let t = ((elapsed - section_idx as f64 * SNAP_STAGGER) / SNAP_SECS).clamp(0.0, 1.0);
                                    Self::render_section(ui, label, bucket.utilization.unwrap_or(0.0), bucket.resets_at.as_deref(), t);
                                    section_idx += 1;
                                }
                            }

                            // Rapid repaint during snap-in animation
                            let anim_end = section_idx as f64 * SNAP_STAGGER + SNAP_SECS;
                            if elapsed < anim_end {
                                ui.ctx().request_repaint_after(Duration::from_millis(16));
                            }
                        }
                    }

                    // Footer (only when we have fetched at least once)
                    if let Some(t) = self.last_fetch {
                        let footer_alpha = self.data_arrived_at.map_or(1.0_f32, |arrived| {
                            let e = arrived.elapsed().as_secs_f64();
                            ((e - 0.3) * 4.0).clamp(0.0, 1.0) as f32
                        });
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(updated_ago(t))
                                .color(with_alpha(FOOTER_DIM, footer_alpha))
                                .size(10.0),
                        );
                    }
                });

                // Resize window height to fit content (with minimum to avoid jump on load)
                let used_h = (content.response.rect.height() + PADDING * 2.0).max(MIN_HEIGHT);
                if (used_h - self.last_height).abs() > 0.5 {
                    self.last_height = used_h;
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(186.0, used_h)));
                }
            });
    }
}

impl UsageApp {
    fn render_loading(ui: &mut egui::Ui, ctx: &egui::Context) {
        const PHRASES: &[&str] = &[
            "Warming up...",
            "Opening cookie jar...",
            "Counting tokens...",
            "Harmonizing...",
            "Consulting the oracle...",
            "Crunching numbers...",
            "Reticulating splines...",
            "Almost there...",
        ];

        let time = ctx.input(|i| i.time);
        let colors = [BAR_BLUE, BAR_YELLOW, BAR_RED];

        // Center the bars + phrase vertically
        let bar_block_h = colors.len() as f32 * (BAR_H + 12.0) + 24.0;
        let available = ui.available_height();
        ui.add_space(((available - bar_block_h) / 2.0).max(0.0));

        for (i, &color) in colors.iter().enumerate() {
            ui.add_space(6.0);
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(BAR_W, BAR_H),
                egui::Sense::hover(),
            );
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 2.0, BAR_BG);

            // Staggered sine wave — each bar fills and empties smoothly, offset in phase
            let phase = time / 2.0 - i as f64 * 0.2;
            let t = (phase.fract() + 1.0).fract();
            let fill = (t * std::f64::consts::PI).sin() as f32;

            if fill > 0.01 {
                let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(BAR_W * fill, BAR_H));
                painter.rect_filled(fill_rect, 2.0, color);
            }
            ui.add_space(6.0);
        }

        // Cycling phrase
        let idx = (time / 2.5) as usize % PHRASES.len();
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(PHRASES[idx])
                .color(DIM)
                .size(11.0),
        );

        ctx.request_repaint_after(Duration::from_millis(30));
    }

    fn render_section(ui: &mut egui::Ui, label: &str, utilization: f64, resets_at: Option<&str>, anim_t: f64) {
        let bar_t = ease_out_back(anim_t);
        let text_alpha = (anim_t * 6.0).min(1.0) as f32;

        let pct = utilization.round().min(100.0);
        let color = bar_color(pct);

        ui.label(
            egui::RichText::new(label)
                .color(with_alpha(FG, text_alpha))
                .font(egui::FontId::new(13.0, egui::FontFamily::Name("bold".into()))),
        );
        let reset_text = time_left(resets_at);
        if !reset_text.is_empty() {
            ui.label(egui::RichText::new(&reset_text).color(with_alpha(DIM, text_alpha)).size(11.0));
        }

        // Progress bar + percentage
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(BAR_W, BAR_H),
                egui::Sense::hover(),
            );
            let painter = ui.painter_at(rect);

            // Bar height bounces — grows from thin, overshoots tall, settles
            let h_scale = bar_t.max(0.0).min(1.15) as f32;
            let draw_h = BAR_H * h_scale;
            let y_off = (BAR_H - draw_h) / 2.0;
            let bar_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(0.0, y_off),
                egui::vec2(BAR_W, draw_h),
            );
            painter.rect_filled(bar_rect, 2.0, BAR_BG);

            // Fill overshoots width then settles — clamp to bar width
            let fill_w = (BAR_W * (pct as f32) / 100.0 * bar_t as f32).min(BAR_W);
            if fill_w > 0.0 {
                let fill_rect = egui::Rect::from_min_size(
                    bar_rect.min,
                    egui::vec2(fill_w, draw_h),
                );
                painter.rect_filled(fill_rect, 2.0, color);
            }

            ui.add_space(3.0);
            // Percentage counts up with the bar
            let display_pct = (pct * bar_t).min(100.0);
            ui.label(
                egui::RichText::new(format!("{:.0}%", display_pct))
                    .color(with_alpha(DIM, text_alpha))
                    .size(11.0),
            );
        });
        ui.add_space(4.0);
    }
}

/// Toggle all-workspaces. Tries KWin D-Bus scripting first (required under
/// KWin Wayland/XWayland where EWMH sticky is ignored), then falls back to
/// EWMH _NET_WM_STATE_STICKY for other window managers.
#[cfg(target_os = "linux")]
fn toggle_all_workspaces(wm_name: String, enable: bool) {
    std::thread::spawn(move || {
        if try_kwin_dbus(&wm_name, enable) {
            return;
        }
        try_ewmh_sticky(&wm_name, enable);
    });
}

/// Use KWin's D-Bus scripting API to set onAllDesktops for our window.
#[cfg(target_os = "linux")]
fn try_kwin_dbus(wm_name: &str, enable: bool) -> bool {
    let script = format!(
        "var w = workspace.windowList();\n\
         for (var i = 0; i < w.length; i++) {{\n\
             if (w[i].caption === \"{}\") {{\n\
                 w[i].onAllDesktops = {};\n\
                 break;\n\
             }}\n\
         }}",
        wm_name, enable
    );

    let tmp = std::env::temp_dir().join(format!("claude_sticky_{}.js", std::process::id()));
    if std::fs::write(&tmp, &script).is_err() {
        return false;
    }

    let script_name = format!("claude_sticky_{}", std::process::id());

    // Load script
    let output = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--dest=org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.loadScript",
            &format!("string:{}", tmp.display()),
            &format!("string:{script_name}"),
        ])
        .output();

    let _ = std::fs::remove_file(&tmp);

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };

    // Parse script ID from dbus-send output ("   int32 N")
    let stdout = String::from_utf8_lossy(&output.stdout);
    let script_id: Option<i32> = stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix("int32 ")?.parse().ok())
        .next();

    let Some(id) = script_id else {
        return false;
    };

    // Run the script
    let _ = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.kde.KWin",
            &format!("/{id}"),
            "org.kde.kwin.Script.run",
        ])
        .output();

    // Unload the script
    let _ = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.unloadScript",
            &format!("string:{script_name}"),
        ])
        .output();

    true
}

/// Fallback: toggle _NET_WM_STATE_STICKY via EWMH (works on non-KWin X11 WMs).
#[cfg(target_os = "linux")]
fn try_ewmh_sticky(wm_name: &str, enable: bool) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::wrapper::ConnectionExt as _;

    let _ = (|| -> Option<()> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[screen_num].root;

        let intern = |name: &[u8]| -> Option<Atom> {
            conn.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom)
        };

        let net_client_list = intern(b"_NET_CLIENT_LIST")?;
        let net_wm_name_atom = intern(b"_NET_WM_NAME")?;
        let utf8_string = intern(b"UTF8_STRING")?;
        let net_wm_state = intern(b"_NET_WM_STATE")?;
        let state_sticky = intern(b"_NET_WM_STATE_STICKY")?;

        let reply = conn
            .get_property(false, root, net_client_list, AtomEnum::WINDOW, 0, 4096)
            .ok()?
            .reply()
            .ok()?;

        for wid in reply.value32()? {
            let name = conn
                .get_property(false, wid, net_wm_name_atom, utf8_string, 0, 256)
                .ok()
                .and_then(|c| c.reply().ok())
                .map(|r| String::from_utf8_lossy(&r.value).to_string());
            if name.as_deref() != Some(wm_name) {
                continue;
            }
            let action = if enable { 1u32 } else { 0u32 };
            let event = ClientMessageEvent {
                response_type: CLIENT_MESSAGE_EVENT,
                format: 32,
                sequence: 0,
                window: wid,
                type_: net_wm_state,
                data: ClientMessageData::from([action, state_sticky, 0, 2, 0]),
            };
            conn.send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            )
            .ok()?;
            conn.flush().ok()?;
            // Sync ensures the X server has processed the event before we
            // drop the connection.  Without this round-trip, the socket can
            // close before the WM acts on the message.
            conn.sync().ok()?;
            return Some(());
        }
        None
    })();
}
