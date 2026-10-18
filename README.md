<p align="center">
  <img src="app-icon.svg" width="96" height="96" alt="Systana logo">
</p>

<h1 align="center">Systana</h1>

<p align="center">
  <b>Rewind your PC.</b><br>
  A tiny Windows app that quietly records CPU, RAM, disk, GPU, network and temperature —<br>
  and tells you in plain words <i>why</i> your PC was slow at 14:05.
</p>

<p align="center">
  <a href="../../releases/latest"><b>⬇ Download</b></a> (Windows · macOS · Linux) ·
  <a href="#features">Features</a> ·
  <a href="#faq">FAQ</a> ·
  <a href="#tiếng-việt">Tiếng Việt</a>
</p>

---

> Systana was called **Sysrewind** until v0.1.1. Installing v0.2.0 or later on Windows removes the old app and keeps your history.

Task Manager shows you **now**. Systana shows you **then**.

Systana runs quietly in the system tray, takes a snapshot of your system every 5 seconds, and keeps the history on your own disk.
When something felt slow — a game stuttered, the fan went wild, a meeting froze — open Systana, pick the time, and see exactly what was going on and which app caused it.

> **Heavily inspired by [Grafana](https://grafana.com) and [Prometheus](https://prometheus.io).**
> They are the tools of choice for watching servers. Systana brings the same idea — record everything, explore any time range later — to a normal Windows PC, with zero setup.

## Features

- **History, not just live numbers.** CPU, RAM, disk, GPU, network and temperatures, kept for 7 to 365 days.
- **Which app did it.** Per-app CPU, RAM, disk and GPU usage, with real app icons. All of Chrome's processes count as one Chrome.
- **Plain-language insights.** *"Heavy load for 7 min. CPU peaked at 87%, mostly ffmpeg (61%)."* Systana finds periods of high CPU, nearly full RAM, busy disk and high temperature on its own.
- **Drag to investigate.** Select any part of the chart to see the busiest apps in that window.
- **Temperatures that just work.** CPU (Intel and AMD Ryzen) and disk temperatures out of the box, plus NVIDIA GPUs.
- **Tiny and private.** ~5 MB installer, about 10–30 MB of RAM in the background. No account, no cloud, no telemetry — your data never leaves your PC.
- **English and Vietnamese.** Dark and light themes.

## Install

Get the file for your system from the **[latest release](../../releases/latest)**:

| System | File | How |
|---|---|---|
| **Windows 10/11** (recommended) | `Systana_x.y.z_x64-setup.exe` | Run it, click **Yes** when Windows asks for permission. Done. |
| Windows (IT / Group Policy) | `Systana_x.y.z_x64_en-US.msi` | Same app as an MSI package, for silent or managed installs: `msiexec /i Systana_x.y.z_x64_en-US.msi /qn` |
| **macOS 11+** (Apple Silicon & Intel) | `Systana_x.y.z_universal.dmg` | Open the DMG, drag Systana to Applications. |
| **Debian / Ubuntu / Mint** | `Systana_x.y.z_amd64.deb` | `sudo apt install ./Systana_x.y.z_amd64.deb` |
| **Fedora / RHEL / openSUSE** | `Systana-x.y.z-1.x86_64.rpm` | `sudo dnf install ./Systana-x.y.z-1.x86_64.rpm` |
| **Any Linux** | `Systana_x.y.z_amd64.AppImage` | `chmod +x Systana_*.AppImage` then run it |

Systana opens, starts recording, and starts automatically when you log in.
Close the window any time — it keeps recording in the system tray / menu bar. To quit completely, use the tray icon → **Quit**.

> **Windows: "Windows protected your PC"?** The installer is not code-signed yet, so SmartScreen may warn about a new app.
> Click **More info → Run anyway**. You can compare the file's SHA-256 with the one listed on the release page.
>
> **macOS: "Systana is damaged" or "can't be opened"?** The app is not notarized yet. After moving it to Applications, run once:
> `xattr -dr com.apple.quarantine /Applications/Systana.app`

### What works where

| | Windows | Linux | macOS |
|---|---|---|---|
| CPU, RAM, network, per-app history, insights | ✅ | ✅ | ✅ |
| Disk throughput | ✅ physical disk | ✅ sum of app I/O | ✅ sum of app I/O |
| GPU usage (per app) | ✅ | — | — |
| CPU temperature | ✅ Intel & AMD Ryzen (PawnIO) | ✅ kernel sensors (`lm-sensors`) | ⚠️ some Macs |
| Disk temperature | ✅ SMART | ✅ NVMe / `drivetemp` | ⚠️ some Macs |
| GPU temperature | ✅ NVIDIA | ✅ NVIDIA, AMD, Intel | — |
| Real app icons | ✅ | letter tiles | letter tiles |

Windows is where Systana is developed and tested most. Linux and macOS builds are new — please [report anything odd](../../issues).

## How to use it

| I want to… | Do this |
|---|---|
| See what happened in the last hour | Open Systana — **Overview** shows the last hour by default |
| Look at yesterday afternoon | **Custom** → pick *from* and *to* → **Apply** |
| Find out why it was slow | Check **Insights**, then click **View period** |
| See which app was heavy at a moment | Drag across the chart; the table below updates |
| Follow one app over time | **Apps** → click the app |
| Change history length or pause | **Settings** |

## FAQ

**Why does the installer need administrator rights?**
Only for temperatures. Windows doesn't let normal apps read CPU temperature. The installer adds the signed, open-source [PawnIO](https://pawnio.eu) driver (the one LibreHardwareMonitor uses) and a small background task that reads temperatures. You are asked **once**, at install time. The Systana window itself never runs as administrator.

**Is it safe?**
The code is open — read it, build it yourself, or check [SECURITY.md](SECURITY.md) for exactly what runs with elevated rights and how it is locked down.

**Does it slow my PC down?**
No. The background recorder uses about 10 MB of RAM and well under 1% CPU. The window is fully closed (not just hidden) when you close it.

**Where is my data? How big does it get?**
In `%APPDATA%\com.systana.app\metrics.db`. Roughly 4–5 MB per day the PC is on (about 400 MB for the default 90 days; choose 30 days in Settings to keep it near 150 MB). Old data is deleted automatically after the period you choose (90 days by default).

**No temperature showing?**
See **Settings → Temperature**. Most often the installer's permission prompt was declined — reinstall and click **Yes**. Some CPUs (older AMD, non-Ryzen) and non-NVIDIA GPUs are not supported yet.

**How do I uninstall?**
*Settings → Apps → Installed apps → Systana → Uninstall*. This removes the app, the background task and the temperature data. The PawnIO driver is left in place because other hardware tools may use it; uninstall it separately if you don't need it.

## Similar projects

Systana is not the only tool in this space, and that's good. Pick what fits you:

- **[HardwareVisualizer](https://github.com/shm11C3/HardwareVisualizer)** — cross-platform hardware monitor with history and rich dashboards (Tauri, GPL-3.0).
- **[LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor)** — the reference for live sensor readings on Windows.
- **[Netdata](https://www.netdata.cloud)** and **Grafana + Prometheus** — powerful monitoring stacks, built mainly for servers.

Systana focuses on one thing: *answering "what happened on my PC, and why?" for anyone, with zero setup.*

## Build from source

Prerequisites: [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 20+, Visual Studio C++ Build Tools.

```sh
npm install
powershell -File scripts/fetch-pawnio.ps1   # downloads and verifies the signed PawnIO installer
npm run tauri dev                           # run with hot reload
npm run tauri build                         # installer in src-tauri/target/release/bundle/nsis
```

Tests and lint: `cd src-tauri && cargo test --lib && cargo clippy --all-targets -- -D warnings`.

| Part | Tech |
|---|---|
| App shell | [Tauri 2](https://tauri.app) — Rust + the system WebView2 |
| Metrics | [sysinfo](https://github.com/GuillaumeGomez/sysinfo) + Windows performance counters (GPU engines, physical disk) |
| Temperatures | PawnIO (Intel MSR / AMD SMN) and SMART in an elevated helper, NVIDIA NVML, LibreHardwareMonitor if running |
| Storage | SQLite — one row per 5 s for the system, per-minute averages for the busiest apps |
| UI | Vanilla JS + [uPlot](https://github.com/leeoniya/uPlot), Geist font |

## Roadmap

- [ ] Smaller history: roll old 5-second samples up into 1-minute averages
- [ ] Alerts (e.g. "RAM above 90% for 5 minutes")
- [ ] CSV export
- [ ] Per-core temperatures, AMD and Intel GPU temperatures
- [ ] Code-signed installer, notarized macOS app, auto-update
- [ ] GPU usage and app icons on Linux and macOS

## Contributing

Bug reports, ideas and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).
Found a security issue? Please report it privately, as described in [SECURITY.md](SECURITY.md).

## Privacy

Systana does not send any data anywhere. There is no telemetry, no account and no network access.
Everything it records stays on your computer (see the FAQ for the exact folder) and is deleted when it is older than the history length you choose.

## Code signing policy

Windows installers are built from this repository by GitHub Actions ([release workflow](.github/workflows/release.yml)); nothing is built on a personal machine.
Code signing is being set up; until then, releases are unsigned and their SHA-256 hashes are listed on each release page.

| Role | Members |
|---|---|
| Authors / committers | [@nocmtriosmith-h1](https://github.com/nocmtriosmith-h1) |
| Reviewers (pull requests from contributors) | [@nocmtriosmith-h1](https://github.com/nocmtriosmith-h1) |
| Release approvers | [@nocmtriosmith-h1](https://github.com/nocmtriosmith-h1) |

The installer bundles the unmodified, upstream-signed [PawnIO](https://pawnio.eu) driver installer (see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).

## License

[MIT](LICENSE) — free to use, modify, fork and share. Bundled third-party components keep their own licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

---

## Tiếng Việt

**Systana** — xem lại máy tính của bạn đã làm gì, vào lúc nào.

Phần mềm nhỏ chạy ngầm trên Windows, ghi lại CPU, RAM, ổ đĩa, GPU, mạng và nhiệt độ mỗi 5 giây. Khi máy bị lag, mở Systana ra, chọn đúng lúc đó và xem **app nào gây ra**, giải thích bằng lời dễ hiểu.
Ý tưởng lấy cảm hứng lớn từ **Grafana** và **Prometheus** — nhưng dành cho máy tính cá nhân, không cần cài đặt phức tạp.

**Cài đặt:**

1. Vào mục [Releases](../../releases/latest), tải file theo hệ điều hành:
   - **Windows:** `Systana_x.y.z_x64-setup.exe` → mở file, bấm **Yes** khi Windows hỏi quyền (chỉ hỏi 1 lần, để đọc nhiệt độ).
   - **macOS:** file `.dmg` → kéo Systana vào Applications.
   - **Ubuntu/Debian:** file `.deb` → `sudo apt install ./Systana_x.y.z_amd64.deb`. Linux khác: `.rpm` hoặc `.AppImage`.
2. Xong. Systana tự ghi và tự chạy khi bật máy.

Nếu Windows hiện *"Windows protected your PC"*: bấm **More info → Run anyway** (do bộ cài chưa có chữ ký số).
Nếu macOS báo app *"bị hỏng"*: chạy `xattr -dr com.apple.quarantine /Applications/Systana.app` một lần.

**Tính năng chính:** lịch sử 7–365 ngày · biết app nào dùng nhiều tài nguyên lúc nào · mục *Nhận định* tự tìm lúc máy chạy nặng, RAM đầy, máy nóng · nhiệt độ CPU và ổ đĩa có sẵn · dữ liệu chỉ nằm trên máy bạn.
