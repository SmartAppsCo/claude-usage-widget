# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).


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
