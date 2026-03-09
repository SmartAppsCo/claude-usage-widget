# Claude Usage Widget

Claude's usage page is buried in settings and requires you to open your browser every time you want to check how much of your plan you've used. This widget sits on your desktop and shows your current session and weekly usage at a glance — pulled from the same data shown at https://claude.ai/settings/usage — so you always know where you stand before hitting a rate limit.

Especially useful for heavy Claude Code users who burn through their allocation quickly and want to keep an eye on remaining capacity without breaking their workflow.

<p align="center">
  <img src="images/screenshot.png" alt="claude-usage-widget screenshot" width="200">
</p>

## Prerequisites

- Logged into claude.ai in a supported browser (Firefox, Chrome, Brave, Edge, or Safari)

The widget reads your session cookie to fetch usage data — no API key needed.

## Platform Notes

The widget auto-detects your browser and reads cookies directly. Depending on the platform and browser, you may see a one-time permission prompt:

| Platform | Browser | Prompt |
|---|---|---|
| **macOS** | Firefox | None |
| **macOS** | Chrome, Brave, Edge | macOS Keychain password (click "Always Allow" to avoid repeat prompts) |
| **macOS** | Safari | Requires Full Disk Access in System Settings → Privacy & Security |
| **Windows** | Chrome, Edge (v127+) | UAC elevation prompt (needed for App-Bound Encryption) |
| **Windows** | Firefox, Brave | None |
| **Linux** | All | None |

Browsers are tried in order until a valid session is found. On macOS, Firefox is tried first to avoid unnecessary prompts. Use `--browser` to skip auto-detection and target a specific browser.

## Install

Download the latest binary for your platform from the [Releases](https://github.com/SmartAppsCo/claude-usage-widget/releases) page:

| Platform | File |
|---|---|
| Linux (x86_64) | `claude-usage-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Apple Silicon) | `claude-usage-aarch64-apple-darwin.zip` |
| macOS (Intel) | `claude-usage-x86_64-apple-darwin.zip` |
| Windows (x86_64) | `claude-usage-x86_64-pc-windows-msvc.zip` |

Extract the archive. On macOS, double-click `Claude Usage.app` to run (or drag it to Applications). On Linux/Windows, place the binary somewhere in your `PATH`.

Alternatively, build from source with [Cargo](https://rustup.rs/):

```
cargo install --git https://github.com/SmartAppsCo/claude-usage-widget.git --tag v0.5.0
```

On Linux, building from source requires these system packages:

```
sudo apt install libgtk-3-dev libxcb-screensaver0-dev
```

## Options

```
--browser <BROWSER>                Browser to read cookies from (auto-detected if omitted)
                                   (firefox, chrome, brave, edge, safari*) (* macOS only)
--data-dir <PATH>                  Custom browser data directory (requires --browser)
--title <NAME>                     Display name shown in the widget header
```

`--data-dir` is useful for non-standard browser installations or custom profiles where the cookie database isn't in the default location.

When `--title` is omitted, the widget fetches your name from the "What should Claude call you?" setting on your account. You can change this at https://claude.ai/settings/general. Passing `--title` skips that extra API call.

## Behavior

The widget polls usage data every 5 minutes by default. It pauses polling automatically when you're idle (no keyboard/mouse activity) to avoid unnecessary requests. Right-click the widget to adjust the refresh interval.

## Disclaimer

This project is not affiliated with, endorsed by, or associated with Anthropic, PBC. "Claude" and "Anthropic" are trademarks of Anthropic, PBC. All other trademarks are the property of their respective owners. This is an independent, unofficial tool.
