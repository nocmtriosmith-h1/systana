# Contributing to Systana

Thanks for helping! Bug reports, ideas, translations and pull requests are all welcome.

## Reporting a bug

Open an issue with the **Bug report** template. Please include:

- Windows version (Settings → System → About)
- CPU and GPU model
- What you expected and what happened
- If it is about temperatures: the output of `"C:\Program Files\Systana\systana.exe" --sensors --once` run from an **administrator** terminal

## Making a change

1. Fork the repository and create a branch.
2. Set up the tools: [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 20+, Visual Studio C++ Build Tools.
3. Run:

   ```sh
   npm install
   powershell -File scripts/fetch-pawnio.ps1
   npm run tauri dev
   ```

4. Before opening a pull request:

   ```sh
   cd src-tauri
   cargo clippy --all-targets -- -D warnings
   cargo test --lib
   ```

5. Keep pull requests small and focused, and describe *why* the change is needed.

## Project layout

| Path | What lives there |
|---|---|
| `src-tauri/src/collector.rs` | 5-second sampler, per-app minute averages |
| `src-tauri/src/insights.rs` | Detects "PC was under strain" episodes |
| `src-tauri/src/sensors.rs` | Elevated temperature helper (PawnIO, SMART) |
| `src-tauri/src/temps.rs` | Picks the best available temperature source |
| `src-tauri/src/db.rs` | SQLite schema and queries |
| `src-tauri/windows/` | Installer hooks and setup scripts |
| `src/` | UI (vanilla JS + uPlot) |

## Translations

UI strings live in `src/i18n.js`. Copy the `en` block, translate it, and add the language to the selector in `index.html`.

By contributing, you agree that your contributions are licensed under the MIT License.
