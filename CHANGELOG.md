# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).


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
