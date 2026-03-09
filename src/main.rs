#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod api;
mod config;
mod cookies;
mod idle;
mod widget;

use cookies::BrowserKind;

// ---------------------------------------------------------------------------
// Linux: prefer XWayland for native compositor shadows on frameless windows.
//
// On Wayland, undecorated xdg_toplevel surfaces get no drop shadow from the
// compositor (Mutter, KWin, etc.).  Running through XWayland instead gives us
// an X11 managed window that the compositor *does* shadow — the same reason
// Tkinter widgets get shadows on Wayland sessions.
//
// If DISPLAY is set we can use XWayland; otherwise we fall back to native
// Wayland (no shadow, but still functional).
//
// Once the XWayland window is mapped we also poke EWMH properties to make it
// sticky, always-on-top, and hidden from the taskbar.
// ---------------------------------------------------------------------------

/// Returns true if we successfully switched to the X11 backend.
#[cfg(target_os = "linux")]
fn prefer_xwayland() -> bool {
    // Already on X11 — nothing to do, EWMH code will work as-is.
    if std::env::var("WAYLAND_DISPLAY").is_err() {
        return std::env::var("DISPLAY").is_ok();
    }

    // Wayland session, but DISPLAY is set → XWayland is available.
    if std::env::var("DISPLAY").is_ok() {
        // Safety: called before any other threads are spawned.
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
        return true;
    }

    // Pure Wayland, no XWayland — fall back to native Wayland.
    false
}

/// Spawn a background thread that waits for our X11 window to appear, then
/// sets EWMH states (sticky, always-on-top).
#[cfg(target_os = "linux")]
fn set_x11_states(wm_name: String) {
    std::thread::spawn(move || {
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if try_set_x11_states(&wm_name).is_some() {
                return;
            }
        }
    });
}

#[cfg(target_os = "linux")]
fn try_set_x11_states(wm_name: &str) -> Option<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;

    let intern = |name: &[u8]| -> Option<Atom> {
        conn.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom)
    };

    let net_client_list = intern(b"_NET_CLIENT_LIST")?;
    let net_wm_name = intern(b"_NET_WM_NAME")?;
    let utf8_string = intern(b"UTF8_STRING")?;
    let net_wm_state = intern(b"_NET_WM_STATE")?;
    let state_sticky = intern(b"_NET_WM_STATE_STICKY")?;
    let state_above = intern(b"_NET_WM_STATE_ABOVE")?;

    let reply = conn
        .get_property(false, root, net_client_list, AtomEnum::WINDOW, 0, 4096)
        .ok()?
        .reply()
        .ok()?;

    let mut window = None;
    for wid in reply.value32()? {
        let name = conn
            .get_property(false, wid, net_wm_name, utf8_string, 0, 256)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.value).to_string());
        if name.as_deref() == Some(wm_name) {
            window = Some(wid);
            break;
        }
    }
    let window = window?;

    for &state in &[state_sticky, state_above] {
        let event = ClientMessageEvent {
            response_type: CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window,
            type_: net_wm_state,
            data: ClientMessageData::from([1u32, state, 0, 1, 0]),
        };
        conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )
        .ok()?;
    }

    conn.flush().ok()?;
    Some(())
}

// ---------------------------------------------------------------------------
// Desktop integration: auto-install .desktop file and icon on Linux so GNOME
// (and other freedesktop-compliant DEs) show the app icon in the dash/taskbar.
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn xdg_data_dir() -> std::path::PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            cookies::platform::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join(".local/share")
        })
}

#[cfg(target_os = "linux")]
fn desktop_file_path() -> std::path::PathBuf {
    xdg_data_dir().join("applications/claude-usage.desktop")
}

#[cfg(target_os = "linux")]
fn icon_install_path() -> std::path::PathBuf {
    xdg_data_dir().join("icons/hicolor/256x256/apps/claude-usage.png")
}

#[cfg(target_os = "linux")]
fn install_desktop_entry() {
    let desktop_path = desktop_file_path();
    let icon_path = icon_install_path();

    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "claude-usage".into());

    // Skip if already installed and Exec= still points to the current binary.
    if desktop_path.exists() && icon_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&desktop_path) {
            if contents.contains(&format!("Exec={exe}")) {
                return;
            }
        }
    }

    let desktop_content = format!("[Desktop Entry]
Type=Application
Name=Claude Usage
Comment=Desktop widget showing Claude usage stats
Exec={exe}
Icon=claude-usage
Terminal=false
StartupWMClass=claude-usage
StartupNotify=false
Categories=Utility;
");

    if let Some(parent) = desktop_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&desktop_path, desktop_content);

    if let Some(parent) = icon_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&icon_path, include_bytes!("../images/icon.png"));

    // Refresh caches so the DE picks up the new icon immediately.
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["-f", "-t"])
        .arg(icon_path.parent().unwrap().parent().unwrap().parent().unwrap())
        .output();
    let _ = std::process::Command::new("update-desktop-database")
        .arg(desktop_path.parent().unwrap())
        .output();
}

#[cfg(target_os = "linux")]
fn uninstall_desktop_entry() {
    let _ = std::fs::remove_file(desktop_file_path());
    let _ = std::fs::remove_file(icon_install_path());
    eprintln!("Desktop entry and icon removed.");
}

fn fatal_error(msg: &str) -> ! {
    eprintln!("{msg}");
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::UI::WindowsAndMessaging::*;
        let text: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        let caption: Vec<u16> = "Claude Usage\0".encode_utf16().collect();
        unsafe {
            MessageBoxW(None, PCWSTR(text.as_ptr()), PCWSTR(caption.as_ptr()), MB_OK | MB_ICONERROR);
        }
    }
    std::process::exit(1);
}

fn print_usage() {
    eprintln!("Usage: claude-usage [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --browser <firefox|chrome|brave|edge>  Browser to read cookies from");
    eprintln!("  --data-dir <PATH>           Browser data directory");
    eprintln!("  --title <NAME>              Widget title (default: Plan Usage)");
    eprintln!("  --uninstall                 Remove desktop entry and icon");
    eprintln!("  --help                      Show this help");
}

fn detect_browsers_or_exit() -> (Option<BrowserKind>, Option<Vec<BrowserKind>>) {
    let found = cookies::detect_browsers("claude.ai");
    if found.is_empty() {
        fatal_error("No claude.ai session found in any supported browser.");
    }
    if found.len() == 1 {
        let b = *found.keys().next().unwrap();
        (Some(b), None)
    } else {
        let mut browsers: Vec<BrowserKind> = found.keys().copied().collect();
        browsers.sort_by_key(|b| match b {
            BrowserKind::Firefox => 0,
            BrowserKind::Chrome => 1,
            BrowserKind::Brave => 2,
            BrowserKind::Edge => 3,
        });
        (None, Some(browsers))
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    let use_x11 = prefer_xwayland();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut browser: Option<BrowserKind> = None;
    let mut data_dir: Option<String> = None;
    let mut title = String::from("Plan Usage");
    let mut title_explicit = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--uninstall" => {
                #[cfg(target_os = "linux")]
                uninstall_desktop_entry();
                #[cfg(not(target_os = "linux"))]
                eprintln!("--uninstall is only supported on Linux");
                std::process::exit(0);
            }
            "--browser" => {
                i += 1;
                if i >= args.len() {
                    fatal_error("Error: --browser requires a value");
                }
                browser = Some(match args[i].as_str() {
                    "firefox" => BrowserKind::Firefox,
                    "chrome" => BrowserKind::Chrome,
                    "brave" => BrowserKind::Brave,
                    "edge" => BrowserKind::Edge,
                    other => {
                        fatal_error(&format!("Error: unknown browser '{other}' (use firefox, chrome, brave, or edge)"));
                    }
                });
            }
            "--data-dir" => {
                i += 1;
                if i >= args.len() {
                    fatal_error("Error: --data-dir requires a value");
                }
                data_dir = Some(args[i].clone());
            }
            "--title" => {
                i += 1;
                if i >= args.len() {
                    fatal_error("Error: --title requires a value");
                }
                title = args[i].clone();
                title_explicit = true;
            }
            other => {
                fatal_error(&format!("Error: unknown argument '{other}'"));
            }
        }
        i += 1;
    }

    if data_dir.is_some() && browser.is_none() {
        fatal_error("Error: --data-dir requires --browser");
    }

    let config = config::Config::load();

    // Auto-install .desktop file and icon on Linux (idempotent).
    #[cfg(target_os = "linux")]
    install_desktop_entry();

    let (browser, picker_options) = if let Some(b) = browser {
        // Explicit --browser flag
        let cookies = cookies::read_cookies(b, "claude.ai", data_dir.as_deref());
        match cookies {
            Ok(ref c) if c.contains_key("sessionKey") => {}
            Ok(_) => {
                fatal_error(&format!("No claude.ai session found in {b}."));
            }
            Err(e) => {
                fatal_error(&format!("Error reading {b} cookies: {e}"));
            }
        }
        (Some(b), None)
    } else {
        detect_browsers_or_exit()
    };

    // Detach from the terminal so the shell prompt returns immediately.
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn fork() -> i32;
            fn setsid() -> i32;
        }
        unsafe {
            let pid = fork();
            if pid > 0 {
                std::process::exit(0); // parent exits
            }
            if pid == 0 {
                setsid(); // child starts a new session
            }
            // pid < 0: fork failed, just continue in foreground
        }
    }

    // Give each instance a unique X11 window name so the EWMH thread can
    // find exactly its own window when multiple instances are running.
    let wm_name = format!("Claude Usage {}", std::process::id());

    let app = widget::UsageApp::new(browser, data_dir, picker_options, title, title_explicit, wm_name.clone(), config);

    use eframe::egui;

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../images/icon.png"))
        .expect("Failed to load icon");

    let viewport = egui::ViewportBuilder::default()
        .with_decorations(false)
        .with_inner_size([186.0, 274.0])
        .with_always_on_top()
        .with_icon(icon);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // On Linux with X11, set EWMH states once the window appears.
    #[cfg(target_os = "linux")]
    if use_x11 {
        set_x11_states(wm_name.clone());
    }

    eframe::run_native(
        &wm_name,
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "noto_sans".to_owned(),
                egui::FontData::from_static(include_bytes!("../fonts/NotoSans-Regular.ttf")),
            );
            fonts.font_data.insert(
                "noto_sans_bold".to_owned(),
                egui::FontData::from_static(include_bytes!("../fonts/NotoSans-Bold.ttf")),
            );
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "noto_sans".to_owned());
            fonts
                .families
                .insert(
                    egui::FontFamily::Name("bold".into()),
                    vec!["noto_sans_bold".to_owned()],
                );
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app))
        }),
    )
    .expect("Failed to start eframe");
}
