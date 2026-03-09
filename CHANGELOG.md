# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).


## [0.6.0] - 2026-03-09

### Added

- Session cookies cached to disk after first successful API call — subsequent launches skip browser DB access, decryption, keychain prompts, and UAC elevation entirely
- On Windows, mid-session cookie expiry shows a context-aware elevation dialog before re-reading from the browser

### Changed

- Windows UAC elevation deferred until the moment Chromium cookies are needed (not at startup)
- Removed `has_v20_cookies` pre-check — always elevate for Chromium browsers on Windows since the Restart Manager needs admin regardless


## [0.5.2] - 2026-03-09

### Fixed

- Windows UAC dialog and README now consistently explain both reasons for elevation (DB locks + App-Bound Encryption)


## [0.5.1] - 2026-03-09

### Fixed

- Windows: cookie database reads broken by exclusive file locks (restored Restart Manager with deferred UAC elevation)


## [0.5.0] - 2026-03-09

### Added

- Safari browser support on macOS (reads `.binarycookies` format)
- Native permission dialogs before macOS Keychain and Safari Full Disk Access prompts
- macOS builds packaged as `.app` bundles (no more Terminal opening on double-click)
- GUI error dialogs on all platforms (macOS via osascript, Linux via zenity/kdialog, Windows via MessageBox)
- Windows explanation dialog before UAC elevation prompt

### Changed

- Cookie databases opened in SQLite immutable mode to avoid WAL/file lock conflicts
- Browser detection is now sequential (Firefox first, then Chromium browsers, Safari last) instead of parallel
- macOS Chromium keychain explanation only shown once per binary path (persisted in config)
- `fork()` detach limited to Linux only (avoids ObjC runtime crash on macOS)

### Fixed

- SQLite "database is locked" error when Firefox holds WAL lock on macOS
- Paths with spaces (e.g. `~/Library/Application Support/...`) failing SQLite URI parsing


## [0.4.0] - 2026-03-09

### Added

- Windows v20 App-Bound Encryption support (Chrome/Edge 127+) via SYSTEM impersonation and double-DPAPI decryption
- Automatic UAC self-elevation when v20-encrypted cookies are detected (no manual "Run as Administrator" needed)
- Windows Restart Manager integration to release file locks held by Chrome/Edge on the cookie database

### Changed

- Cookie databases are now opened read-only instead of being copied to a temp file
- Removed `tempfile` dependency

### Fixed

- Windows path separator issues causing cookie database lookups to fail
- Right-click context menu white-on-white text on Windows (now uses explicit dark background)


## [0.3.1] - 2026-03-09

### Added

- Pre-built binaries for Linux, macOS (Intel + Apple Silicon), and Windows attached to GitHub releases
- Linux build-from-source dependencies documented in README


## [0.3.0] - 2026-03-09

### Added

- Persistent config file saves refresh interval, always-on-top, and all-workspaces preferences across launches
- Config stored at OS-specific user config directory (`XDG_CONFIG_HOME`, `~/Library/Application Support`, `%APPDATA%`)

### Changed

- Browser detection now runs in parallel (one thread per browser) for faster startup


## [0.2.0] - 2026-03-09

### Added

- Windows support: Chrome/Brave cookie decryption via DPAPI + AES-256-GCM
- Animated loading screen with colored progress bars and cycling status phrases
- Minimum window height to prevent resize jump on initial load

### Changed

- Cookies read once per fetch cycle instead of twice (performance)
- Keyring password cached to avoid repeated subprocess spawns on Linux
- Deduplicated shared code across cookie modules


## [0.1.1] - 2026-03-09

### Added

- App icon shown in taskbar/dock on all platforms (Linux, macOS, Windows)
- Auto-install `.desktop` file and icon on Linux for GNOME dash/app list
- `--uninstall` flag to remove desktop entry and icon on Linux
- Embed icon in Windows `.exe` via build script for file explorer and pinned taskbar

### Changed

- App now appears in Linux taskbar (removed skip-taskbar window hint)


## [0.1.0] - 2026-03-08

Initial release.

### Features

- Desktop widget showing Claude Pro/Team usage (current session + weekly limits)
- Auto-detects browser session from Firefox, Chrome, or Brave
- Auto-refreshes usage data (configurable interval via right-click menu)
- Idle detection pauses polling when inactive
- Always-on-top and all-workspaces toggles
- Frameless, draggable window with XWayland support for compositor shadows
- Fetches account name from Claude settings for the widget title
