//! Picks the best available temperature source for the platform.
//!
//! Windows (no admin needed in the app itself):
//! 1. Systana's elevated helper (`--sensors`, PawnIO + SMART), see sensors.rs.
//! 2. LibreHardwareMonitor's WMI provider (root\LibreHardwareMonitor) when the user runs LHM.
//! 3. NVIDIA NVML for NVIDIA GPU temperature.
//! 4. The ACPI thermal zone, only once it has proven to move (many machines report a constant).
//!
//! Linux and macOS: the kernel / SMC sensors exposed through sysinfo (hwmon on Linux, no root),
//! plus NVML for NVIDIA GPUs.

use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;

#[derive(Default, Clone)]
pub struct Temps {
    pub cpu: Option<f64>,
    pub gpu: Option<f64>,
    pub disk: Option<f64>,
    /// Where the CPU temperature came from, shown in Settings.
    pub source: String,
}

/// Highest valid reading per group, preferring sensors whose name matches `preferred`
/// (case-insensitive substring), e.g. the CPU package over individual cores.
fn pick<'a>(readings: impl Iterator<Item = (&'a str, f64)> + Clone, preferred: &[&str]) -> Option<f64> {
    let valid = readings.filter(|(_, v)| v.is_finite() && *v > 0.0 && *v < 150.0);
    preferred
        .iter()
        .find_map(|p| {
            let p = p.to_ascii_lowercase();
            valid.clone().find(|(n, _)| n.to_ascii_lowercase().contains(&p)).map(|(_, v)| v)
        })
        .or_else(|| valid.map(|(_, v)| v).reduce(f64::max))
}

fn read_nvml(nvml: &Option<Nvml>) -> Option<f64> {
    let nvml = nvml.as_ref()?;
    let count = nvml.device_count().ok()?;
    (0..count)
        .filter_map(|i| nvml.device_by_index(i).ok())
        .filter_map(|d| d.temperature(TemperatureSensor::Gpu).ok())
        .map(f64::from)
        .reduce(f64::max)
}

#[cfg(windows)]
pub use windows_impl::TempReader;
#[cfg(not(windows))]
pub use unix_impl::TempReader;

#[cfg(windows)]
mod windows_impl {
    use super::{pick, read_nvml, Temps};
    use nvml_wrapper::Nvml;
    use serde::Deserialize;
    use std::time::{Duration, Instant};
    use wmi::{COMLibrary, WMIConnection};

    /// LHM is optional: when it is not running, try to connect again after this long.
    const LHM_RETRY: Duration = Duration::from_secs(60);
    /// ACPI zone readings must span at least this many degrees before we trust them.
    const ACPI_MIN_SPREAD: f64 = 2.0;

    #[derive(Deserialize)]
    #[serde(rename = "Sensor")]
    #[serde(rename_all = "PascalCase")]
    struct LhmSensor {
        identifier: String,
        name: String,
        value: f32,
    }

    pub struct TempReader {
        com: Option<COMLibrary>,
        lhm: Option<WMIConnection>,
        lhm_next_try: Instant,
        nvml: Option<Nvml>,
        acpi_min: f64,
        acpi_max: f64,
    }

    impl TempReader {
        pub fn new() -> Self {
            Self {
                com: COMLibrary::new().ok(),
                lhm: None,
                lhm_next_try: Instant::now(),
                nvml: Nvml::init().ok(),
                acpi_min: f64::MAX,
                acpi_max: f64::MIN,
            }
        }

        /// `acpi_celsius`: the ACPI thermal zone reading from PDH, if any.
        pub fn read(&mut self, acpi_celsius: Option<f64>) -> Temps {
            let helper = crate::sensors::read_latest();
            let mut t = self.read_lhm();
            if t.cpu.is_some() || t.gpu.is_some() || t.disk.is_some() {
                t.source = "LibreHardwareMonitor".into();
            }
            if let Some(h) = &helper {
                if h.cpu.is_some() {
                    t.cpu = h.cpu;
                    t.source = h.source.clone();
                }
                t.disk = h.disk.or(t.disk);
            }
            if t.gpu.is_none() {
                t.gpu = read_nvml(&self.nvml);
            }
            if t.cpu.is_none() {
                t.cpu = self.trusted_acpi(acpi_celsius);
                if t.cpu.is_some() {
                    t.source = "ACPI".into();
                }
            }
            t
        }

        fn read_lhm(&mut self) -> Temps {
            if self.lhm.is_none() && Instant::now() >= self.lhm_next_try {
                self.lhm_next_try = Instant::now() + LHM_RETRY;
                if let Some(com) = self.com {
                    self.lhm = WMIConnection::with_namespace_path("root\\LibreHardwareMonitor", com).ok();
                }
            }
            let Some(conn) = &self.lhm else { return Temps::default() };
            let sensors: Vec<LhmSensor> =
                match conn.raw_query("SELECT Identifier, Name, Value FROM Sensor WHERE SensorType = 'Temperature'") {
                    Ok(s) => s,
                    Err(_) => {
                        // LHM was closed: drop the connection and retry later.
                        self.lhm = None;
                        return Temps::default();
                    }
                };
            classify_lhm(&sensors)
        }

        fn trusted_acpi(&mut self, celsius: Option<f64>) -> Option<f64> {
            let c = celsius.filter(|c| (1.0..130.0).contains(c))?;
            self.acpi_min = self.acpi_min.min(c);
            self.acpi_max = self.acpi_max.max(c);
            (self.acpi_max - self.acpi_min >= ACPI_MIN_SPREAD).then_some(c)
        }
    }

    /// LHM identifiers look like `/intelcpu/0/temperature/0`, `/gpu-nvidia/0/temperature/0`, `/nvme/0/temperature/0`.
    fn classify_lhm(sensors: &[LhmSensor]) -> Temps {
        let kind = |s: &LhmSensor| s.identifier.trim_start_matches('/').split('/').next().unwrap_or("").to_ascii_lowercase();
        let group = |pred: fn(&str) -> bool| sensors.iter().filter(move |s| pred(&kind(s))).map(|s| (s.name.as_str(), s.value as f64));
        Temps {
            cpu: pick(group(|k| k.contains("cpu")), &["CPU Package", "Core Max", "Core (Tctl/Tdie)"]),
            gpu: pick(group(|k| k.starts_with("gpu")), &["GPU Core"]),
            disk: pick(group(|k| matches!(k, "nvme" | "hdd" | "ssd" | "storage" | "ata")), &[]),
            source: String::new(),
        }
    }
}

#[cfg(not(windows))]
mod unix_impl {
    use super::{pick, read_nvml, Temps};
    use nvml_wrapper::Nvml;
    use sysinfo::Components;

    pub struct TempReader {
        components: Components,
        nvml: Option<Nvml>,
    }

    impl TempReader {
        pub fn new() -> Self {
            Self { components: Components::new_with_refreshed_list(), nvml: Nvml::init().ok() }
        }

        /// Labels look like `coretemp Package id 0`, `k10temp Tctl`, `nvme Composite`, `amdgpu edge` (Linux)
        /// or `CPU Die`, `GPU`, `NAND` (macOS).
        pub fn read(&mut self, _acpi_celsius: Option<f64>) -> Temps {
            self.components.refresh(true);
            let readings: Vec<(String, f64)> = self
                .components
                .iter()
                .filter_map(|c| c.temperature().map(|t| (c.label().to_ascii_lowercase(), t as f64)))
                .collect();
            let group = |keys: &'static [&'static str]| {
                readings.iter().filter(move |(l, _)| keys.iter().any(|k| l.contains(k))).map(|(l, v)| (l.as_str(), *v))
            };
            let cpu = pick(group(&["coretemp", "k10temp", "zenpower", "cpu", "package", "tctl", "tdie", "pmu tdie"]), &["package", "tctl", "tdie", "cpu die"]);
            let gpu = pick(group(&["amdgpu", "nouveau", "radeon", "i915", "xe", "gpu"]), &["edge", "gpu"]).or_else(|| read_nvml(&self.nvml));
            let disk = pick(group(&["nvme", "drivetemp", "nand", "ssd"]), &["composite"]);
            Temps { cpu, gpu, disk, source: if cpu.is_some() { "sysinfo".into() } else { String::new() } }
        }
    }
}
