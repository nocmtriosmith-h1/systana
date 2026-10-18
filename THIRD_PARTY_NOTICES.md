# Third-party notices

Systana is licensed under the MIT License (see `LICENSE`). It includes or uses the following third-party software.

## Bundled with the installer

### PawnIO (driver and installer)

- Project: https://github.com/namazso/PawnIO — installer: https://github.com/namazso/PawnIO.Setup
- Version bundled: 2.2.0 (`PawnIO_setup.exe`, signed by namazso.eu, SHA-256 `1F519A22E47187F70A1379A48CA604981C4FCF694F4E65B734AAA74A9FBA3032`)
- License: GPL-2.0-or-later, with an exception for independent programs that communicate with PawnIO only through its device I/O control interface.
- Systana does not modify PawnIO. It runs the unmodified official installer and talks to the driver only through PawnIOLib (below), which uses DeviceIoControl.
- Source code: https://github.com/namazso/PawnIO and https://github.com/namazso/PawnIO.Setup

The PawnIO installer also places PawnIOLib.dll (LGPL-2.1-or-later) and modules from PawnIO.Modules (LGPL-2.1-or-later, https://github.com/namazso/PawnIO.Modules) under `C:\Program Files\PawnIO`. Systana loads `PawnIOLib.dll` and the `IntelMSR.bin` / `AMDFamily17.bin` modules from there at runtime.

## Used at runtime if present

- NVIDIA NVML (`nvml.dll`, part of the NVIDIA driver) — read GPU temperature.
- LibreHardwareMonitor (MPL-2.0, https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) — if the user runs it, Systana reads its WMI sensors. Not bundled.

## Compiled into Systana

Rust crates (all MIT and/or Apache-2.0 unless noted): tauri, tauri-plugin-autostart, tauri-plugin-single-instance, serde, serde_json, rusqlite (MIT; bundles SQLite, public domain), sysinfo, wmi, nvml-wrapper, libloading (ISC).

JavaScript packages: uPlot (MIT), @tauri-apps/api (MIT/Apache-2.0), @fontsource-variable/geist and @fontsource-variable/geist-mono (Geist fonts, SIL Open Font License 1.1).
