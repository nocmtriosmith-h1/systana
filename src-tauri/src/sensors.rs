//! Elevated sensor helper (`systana.exe --sensors`).
//!
//! The installer registers a scheduled task that runs this mode as SYSTEM at boot. It reads
//! CPU temperature through the PawnIO driver (signed, installed alongside Systana) and disk
//! temperature through SMART, then writes them to a small JSON file that the normal,
//! non-elevated app reads. The UI itself never runs elevated.
//!
//! PawnIO is used only through PawnIOLib.dll (LGPL-2.1, loaded at runtime), which talks to the
//! driver over DeviceIoControl, as the PawnIO license exception requires.

use libloading::{Library, Symbol};
use serde::{Deserialize, Serialize};
use std::ffi::{c_char, c_void, CString};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INTERVAL: Duration = Duration::from_secs(5);
/// Readings older than this are ignored by the app (helper stopped or crashed).
pub const MAX_AGE_SECS: i64 = 20;

const MSR_IA32_TEMPERATURE_TARGET: u64 = 0x1A2;
const MSR_IA32_PACKAGE_THERM_STATUS: u64 = 0x1B1;
/// AMD Zen: THM_TCON_CUR_TMP in the SMN address space.
const SMN_THM_TCON_CUR_TMP: u64 = 0x0005_9800;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Reading {
    pub ts: i64,
    pub cpu: Option<f64>,
    pub disk: Option<f64>,
    pub source: String,
}

/// `C:\ProgramData\Systana\sensors.json`: written by SYSTEM, readable by every user.
pub fn reading_path() -> PathBuf {
    let base = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    base.join("Systana").join("sensors.json")
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// For the normal app: the helper's latest reading, if it is fresh.
pub fn read_latest() -> Option<Reading> {
    let text = std::fs::read_to_string(reading_path()).ok()?;
    let r: Reading = serde_json::from_str(&text).ok()?;
    (now_secs() - r.ts <= MAX_AGE_SECS).then_some(r)
}

// ---------- PawnIO ----------

type Handle = *mut c_void;
type OpenFn = unsafe extern "system" fn(*mut Handle) -> i32;
type LoadFn = unsafe extern "system" fn(Handle, *const u8, usize) -> i32;
type ExecFn = unsafe extern "system" fn(Handle, *const c_char, *const u64, usize, *mut u64, usize, *mut usize) -> i32;
type CloseFn = unsafe extern "system" fn(Handle) -> i32;

#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(attrs: *mut c_void, initial_owner: i32, name: *const u16) -> Handle;
    fn WaitForSingleObject(handle: Handle, ms: u32) -> u32;
    fn ReleaseMutex(handle: Handle) -> i32;
}

enum Cpu {
    Intel,
    /// AMD family 17h–1Ah (Zen).
    AmdZen,
}

struct PawnIo {
    _lib: Library,
    handle: Handle,
    exec: ExecFn,
    close: CloseFn,
    cpu: Cpu,
    /// Guards SMN access, shared with other hardware tools (LibreHardwareMonitor, HWiNFO…).
    pci_mutex: Handle,
}

fn pawnio_dir() -> PathBuf {
    let pf = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    pf.join("PawnIO")
}

fn detect_cpu() -> Option<Cpu> {
    use std::arch::x86_64::__cpuid;
    let v = __cpuid(0);
    let mut vendor = Vec::with_capacity(12);
    for r in [v.ebx, v.edx, v.ecx] {
        vendor.extend_from_slice(&r.to_le_bytes());
    }
    let sig = __cpuid(1).eax;
    let base = (sig >> 8) & 0xF;
    let family = if base == 0xF { base + ((sig >> 20) & 0xFF) } else { base };
    match vendor.as_slice() {
        b"GenuineIntel" => Some(Cpu::Intel),
        b"AuthenticAMD" if (0x17..=0x1A).contains(&family) => Some(Cpu::AmdZen),
        _ => None,
    }
}

impl PawnIo {
    fn open() -> Result<Self, String> {
        let cpu = detect_cpu().ok_or("unsupported CPU")?;
        let dir = pawnio_dir();
        let module = match cpu {
            Cpu::Intel => "IntelMSR.bin",
            Cpu::AmdZen => "AMDFamily17.bin",
        };
        let blob = std::fs::read(dir.join(module)).map_err(|e| format!("{module}: {e}"))?;
        unsafe {
            // Runs as SYSTEM: resolve PawnIOLib's own dependencies only from its folder and System32,
            // never from PATH or the current directory.
            const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x0000_0100;
            const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;
            let lib: Library = libloading::os::windows::Library::load_with_flags(
                dir.join("PawnIOLib.dll"),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
            .map_err(|e| format!("PawnIOLib.dll: {e}"))?
            .into();
            let open: Symbol<OpenFn> = lib.get(b"pawnio_open\0").map_err(|e| e.to_string())?;
            let load: Symbol<LoadFn> = lib.get(b"pawnio_load\0").map_err(|e| e.to_string())?;
            let exec: ExecFn = *lib.get::<ExecFn>(b"pawnio_execute\0").map_err(|e| e.to_string())?;
            let close: CloseFn = *lib.get::<CloseFn>(b"pawnio_close\0").map_err(|e| e.to_string())?;
            let mut handle: Handle = std::ptr::null_mut();
            let hr = open(&mut handle);
            if hr < 0 {
                return Err(format!("pawnio_open failed: 0x{hr:08X} (needs admin and the PawnIO driver)"));
            }
            let hr = load(handle, blob.as_ptr(), blob.len());
            if hr < 0 {
                close(handle);
                return Err(format!("pawnio_load {module} failed: 0x{hr:08X}"));
            }
            let name: Vec<u16> = "Global\\Access_PCI\0".encode_utf16().collect();
            let pci_mutex = CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
            Ok(Self { _lib: lib, handle, exec, close, cpu, pci_mutex })
        }
    }

    fn call(&self, func: &str, arg: u64) -> Option<u64> {
        let name = CString::new(func).ok()?;
        let input = [arg];
        let mut out = [0u64; 1];
        let mut written = 0usize;
        let hr = unsafe { (self.exec)(self.handle, name.as_ptr(), input.as_ptr(), 1, out.as_mut_ptr(), 1, &mut written) };
        (hr >= 0 && written == 1).then_some(out[0])
    }

    fn cpu_temp(&self) -> Option<f64> {
        match self.cpu {
            Cpu::Intel => {
                // TjMax (bits 23:16) minus the package digital readout (bits 22:16).
                let target = self.call("ioctl_read_msr", MSR_IA32_TEMPERATURE_TARGET)?;
                let tjmax = match (target >> 16) & 0xFF {
                    0 => 100,
                    t => t,
                };
                let status = self.call("ioctl_read_msr", MSR_IA32_PACKAGE_THERM_STATUS)?;
                Some(tjmax as f64 - ((status >> 16) & 0x7F) as f64)
            }
            Cpu::AmdZen => {
                let locked = !self.pci_mutex.is_null() && unsafe { WaitForSingleObject(self.pci_mutex, 100) } == 0;
                let raw = self.call("ioctl_read_smn", SMN_THM_TCON_CUR_TMP);
                if locked {
                    unsafe { ReleaseMutex(self.pci_mutex) };
                }
                // CUR_TEMP in bits 31:21, 0.125 °C per step; bit 19 selects the -49 °C range.
                let raw = raw?;
                let mut t = ((raw >> 21) & 0x7FF) as f64 / 8.0;
                if raw & (1 << 19) != 0 {
                    t -= 49.0;
                }
                Some(t)
            }
        }
    }

    fn source(&self) -> &'static str {
        match self.cpu {
            Cpu::Intel => "PawnIO (Intel)",
            Cpu::AmdZen => "PawnIO (AMD)",
        }
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        unsafe { (self.close)(self.handle) };
    }
}

// ---------- SMART disk temperature (needs admin) ----------

#[derive(Deserialize)]
#[serde(rename = "MSFT_StorageReliabilityCounter")]
#[serde(rename_all = "PascalCase")]
struct ReliabilityCounter {
    temperature: Option<u8>,
}

/// Only reachable as an association of each physical disk; enumerating the class directly returns nothing.
#[derive(Deserialize)]
#[serde(rename = "MSFT_PhysicalDiskToStorageReliabilityCounter")]
struct DiskToCounter {}

#[derive(Deserialize)]
#[serde(rename = "MSFT_PhysicalDisk")]
struct PhysicalDisk {
    #[serde(rename = "__Path")]
    path: String,
}

struct DiskTemps {
    conn: Option<wmi::WMIConnection>,
}

impl DiskTemps {
    fn new() -> Self {
        let conn = wmi::COMLibrary::new()
            .ok()
            .and_then(|com| wmi::WMIConnection::with_namespace_path("root\\Microsoft\\Windows\\Storage", com).ok());
        Self { conn }
    }

    fn max(&self) -> Option<f64> {
        let conn = self.conn.as_ref()?;
        let disks: Vec<PhysicalDisk> = conn.raw_query("SELECT __Path FROM MSFT_PhysicalDisk").ok()?;
        disks
            .iter()
            .filter_map(|d| conn.associators::<ReliabilityCounter, DiskToCounter>(&d.path).ok())
            .flatten()
            .filter_map(|r| r.temperature)
            .filter(|t| *t > 0 && *t < 120)
            .max()
            .map(f64::from)
    }
}

// ---------- helper entry point ----------

fn sample(pawn: &Option<PawnIo>, disks: &DiskTemps) -> Reading {
    let cpu = pawn.as_ref().and_then(|p| p.cpu_temp()).filter(|t| (1.0..130.0).contains(t));
    Reading {
        ts: now_secs(),
        cpu,
        disk: disks.max(),
        source: pawn.as_ref().map(|p| p.source().to_string()).unwrap_or_default(),
    }
}

/// Writes as SYSTEM, so it never follows a link: the folder must already exist (the installer
/// creates it with locked-down permissions) and must not be a junction or symlink, and the temp
/// file is always created fresh rather than opened, so a planted link or hard link is never written through.
fn write_atomically(r: &Reading) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind, Write};
    let path = reading_path();
    let dir = path.parent().ok_or_else(|| Error::from(ErrorKind::NotFound))?;
    let meta = std::fs::symlink_metadata(dir)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(Error::new(ErrorKind::PermissionDenied, "sensor folder is not a plain directory"));
    }
    let tmp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp); // removes a stale file or link itself, never its target
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    f.write_all(&serde_json::to_vec(r).unwrap_or_default())?;
    drop(f);
    std::fs::rename(tmp, path)
}

/// `--sensors`: loop forever. `--sensors --once`: print one reading as JSON and exit (for testing).
pub fn run_helper(once: bool) -> i32 {
    let pawn = match PawnIo::open() {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("PawnIO unavailable: {e}");
            None
        }
    };
    let disks = DiskTemps::new();
    if once {
        println!("{}", serde_json::to_string(&sample(&pawn, &disks)).unwrap_or_default());
        return 0;
    }
    loop {
        let _ = write_atomically(&sample(&pawn, &disks));
        std::thread::sleep(INTERVAL);
    }
}
