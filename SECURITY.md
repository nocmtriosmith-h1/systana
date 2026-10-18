# Security policy

Systana installs a signed kernel driver (PawnIO) and runs a small helper as SYSTEM, so security reports are taken seriously.

## Reporting a vulnerability

Please **do not open a public issue** for security problems.
Use GitHub's private reporting instead: **Security → Report a vulnerability** on this repository.

Include what you found, how to reproduce it, and the Systana version. You should get a reply within 7 days.

## What runs with elevated rights

| Component | Runs as | What it does |
|---|---|---|
| Installer (`Systana_x.y.z_x64-setup.exe`) | Administrator, once | Installs to Program Files, installs PawnIO if missing, registers the **Systana Sensors** task |
| `systana.exe --sensors` | SYSTEM, at boot | Reads CPU temperature through PawnIO (read-only MSR/SMN registers) and disk temperature through SMART; writes `%ProgramData%\Systana\sensors.json` |
| Systana app and window | The signed-in user, **not** elevated | Records metrics, shows the UI |

Hardening in place:

- The SYSTEM task is only registered when Systana is installed under `Program Files`, which ordinary users cannot modify. The installer's elevated scripts also run only from there, so nobody can swap them in a user-writable folder.
- `%ProgramData%\Systana` has inheritance removed: SYSTEM and Administrators have full control, Users can only read. Because any user can create folders in ProgramData, the installer treats an existing folder as untrusted: it removes a junction without following it, resets the owner to Administrators and empties the folder.
- The helper never follows links when it writes: it refuses a folder that is a junction or symlink, does not create the folder itself, and always creates its temp file fresh.
- Uninstall deletes the data folder with `rmdir /s`, which removes links instead of following them.
- App icons are only read from local paths. Network paths (`\\host\share`) are skipped, so Systana never connects to another machine.
- `PawnIOLib.dll` is loaded by absolute path, and its dependencies are resolved only from its own folder and System32.
- The helper only **reads** hardware registers. It never writes MSRs.
- The UI uses a strict Content Security Policy, and every process name is escaped before it is shown.
- No network access, accounts or telemetry. All data stays in `%APPDATA%\com.systana.app`.

## Supported versions

Only the latest release receives security fixes.
