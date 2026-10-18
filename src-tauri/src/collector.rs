//! Background sampler: every 5s records system metrics; every minute stores the top apps.

use crate::{counters, db, temps};
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Networks, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub const INTERVAL_SECS: u64 = 5;
/// Apps kept per minute for each metric (union of CPU, RAM, disk and GPU rankings).
const TOP_PER_METRIC: usize = 5;

#[derive(Serialize, Clone, Default)]
pub struct AppUsage {
    pub name: String,
    pub cpu: f64,
    pub mem: u64,
    pub disk: f64,
    pub gpu: f64,
}

#[derive(Serialize, Clone, Default)]
pub struct Snapshot {
    pub ts: i64,
    pub cpu: f64,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub disk_read: f64,
    pub disk_write: f64,
    pub disk_busy: f64,
    pub net_rx: f64,
    pub net_tx: f64,
    pub gpu: Option<f64>,
    pub gpu_mem: Option<u64>,
    pub cpu_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub disk_temp: Option<f64>,
    pub temp_source: String,
    /// Systana's own footprint (collector process only, not the WebView2 window).
    pub self_mem: u64,
    pub self_cpu: f64,
    pub top: Vec<AppUsage>,
}

#[derive(Default)]
struct MinuteAcc {
    cpu: f64,
    mem: f64,
    mem_max: u64,
    disk: f64,
    gpu: f64,
}

pub type Latest = Arc<Mutex<Option<Snapshot>>>;

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// "chrome.exe" -> "chrome": friendlier, and groups all Chrome processes together.
fn app_name(raw: &str) -> String {
    raw.strip_suffix(".exe").or_else(|| raw.strip_suffix(".EXE")).unwrap_or(raw).to_string()
}

pub fn spawn(db_path: PathBuf, latest: Latest, paused: Arc<AtomicBool>) {
    std::thread::Builder::new()
        .name("collector".into())
        .spawn(move || loop {
            // A database error (disk full, file locked too long...) must not stop recording for good:
            // log it, wait, reopen the database and carry on.
            if let Err(e) = run(db_path.clone(), latest.clone(), paused.clone()) {
                eprintln!("collector error, restarting in 30 s: {e}");
            }
            std::thread::sleep(Duration::from_secs(30));
        })
        .expect("spawn collector thread");
}

fn run(db_path: PathBuf, latest: Latest, paused: Arc<AtomicBool>) -> rusqlite::Result<()> {
    let conn = db::open(&db_path)?;
    let mut sys = System::new();
    let mut networks = Networks::new_with_refreshed_list();
    let counters = counters::Counters::new();
    let mut temp_reader = temps::TempReader::new();
    // The exe path is only needed once per process (for app icons).
    let proc_kind = ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_disk_usage()
        .with_exe(UpdateKind::OnlyIfNotSet);
    sys.refresh_cpu_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, proc_kind);

    let mut minute_bucket = now_secs() / 60;
    let mut minute_ticks = 0u32;
    let mut minute_acc: HashMap<String, MinuteAcc> = HashMap::new();
    let mut last_prune = 0i64;
    // App names whose exe path is already saved this session (for icons).
    let mut known_exes: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut last_tick = Instant::now();

    loop {
        std::thread::sleep(Duration::from_secs(INTERVAL_SECS));
        let elapsed = last_tick.elapsed().as_secs_f64().max(1.0);
        last_tick = Instant::now();
        if paused.load(Ordering::Relaxed) {
            continue;
        }

        sys.refresh_cpu_usage();
        sys.refresh_memory();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, proc_kind);
        networks.refresh(true);
        counters.collect();

        let ts = now_secs();
        let cores = sys.cpus().len().max(1) as f64;
        let (gpu_total, gpu_by_pid) = counters.gpu();

        // Group processes by app name (Chrome has dozens of processes).
        let mut apps: HashMap<String, AppUsage> = HashMap::new();
        let mut proc_disk_total = 0.0;
        for (pid, p) in sys.processes() {
            if pid.as_u32() == 0 {
                continue; // System Idle Process
            }
            let name = app_name(&p.name().to_string_lossy());
            if !known_exes.contains(&name) {
                // Elevated or other users' processes hide their path; try again on a later tick.
                if let Some(exe) = p.exe().and_then(|e| e.to_str()) {
                    db::save_app_exe(&conn, &name, exe)?;
                    known_exes.insert(name.clone());
                }
            }
            let du = p.disk_usage();
            let disk = (du.read_bytes + du.written_bytes) as f64 / elapsed;
            proc_disk_total += disk;
            let a = apps.entry(name.clone()).or_insert_with(|| AppUsage { name, ..Default::default() });
            // sysinfo reports % of one core; Task Manager reports % of the whole CPU.
            a.cpu += p.cpu_usage() as f64 / cores;
            a.mem += p.memory();
            a.disk += disk;
            a.gpu += gpu_by_pid.get(&pid.as_u32()).copied().unwrap_or(0.0);
        }
        for a in apps.values_mut() {
            a.gpu = a.gpu.min(100.0);
        }

        let (net_rx, net_tx) = networks.iter().fold((0u64, 0u64), |(rx, tx), (_, d)| {
            (rx + d.received(), tx + d.transmitted())
        });
        // Without physical-disk counters, approximate throughput from per-process I/O.
        let (disk_read, disk_write, disk_busy) = match counters.disk() {
            Some(d) => (d.read, d.write, d.busy),
            None => (proc_disk_total, 0.0, 0.0),
        };
        let gpu_mem = counters.gpu_mem();
        let temps = temp_reader.read(counters.acpi_celsius());
        let own = sysinfo::get_current_pid().ok().and_then(|pid| sys.process(pid));

        let mut snap = Snapshot {
            ts,
            cpu: sys.global_cpu_usage() as f64,
            mem_used: sys.used_memory(),
            mem_total: sys.total_memory(),
            swap_used: sys.used_swap(),
            disk_read,
            disk_write,
            disk_busy,
            net_rx: net_rx as f64 / elapsed,
            net_tx: net_tx as f64 / elapsed,
            gpu: gpu_total,
            gpu_mem,
            cpu_temp: temps.cpu,
            gpu_temp: temps.gpu,
            disk_temp: temps.disk,
            temp_source: temps.source.clone(),
            self_mem: own.map(|p| p.memory()).unwrap_or(0),
            self_cpu: own.map(|p| p.cpu_usage() as f64 / cores).unwrap_or(0.0),
            top: Vec::new(),
        };

        conn.execute(
            "INSERT OR REPLACE INTO system_samples
             (ts, cpu, mem_used, mem_total, swap_used, disk_read, disk_write, disk_busy, net_rx, net_tx, gpu, gpu_mem,
              cpu_temp, gpu_temp, disk_temp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                snap.ts, snap.cpu, snap.mem_used as i64, snap.mem_total as i64, snap.swap_used as i64,
                snap.disk_read, snap.disk_write, snap.disk_busy, snap.net_rx, snap.net_tx,
                snap.gpu, snap.gpu_mem.map(|v| v as i64), snap.cpu_temp, snap.gpu_temp, snap.disk_temp
            ],
        )?;

        // Flush the previous minute's app averages when the minute changes.
        let bucket = ts / 60;
        if bucket != minute_bucket {
            flush_minute(&conn, minute_bucket * 60, minute_ticks, &minute_acc)?;
            minute_acc.clear();
            minute_ticks = 0;
            minute_bucket = bucket;
        }
        minute_ticks += 1;
        for a in apps.values() {
            let m = minute_acc.entry(a.name.clone()).or_default();
            m.cpu += a.cpu;
            m.mem += a.mem as f64;
            m.mem_max = m.mem_max.max(a.mem);
            m.disk += a.disk;
            m.gpu += a.gpu;
        }

        let mut top: Vec<AppUsage> = apps.into_values().collect();
        top.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        top.truncate(10);
        snap.top = top;
        if let Ok(mut l) = latest.lock() {
            *l = Some(snap);
        }

        if ts - last_prune > 3600 {
            db::prune(&conn, ts)?;
            last_prune = ts;
        }
    }
}

fn flush_minute(conn: &rusqlite::Connection, ts: i64, ticks: u32, acc: &HashMap<String, MinuteAcc>) -> rusqlite::Result<()> {
    if ticks == 0 || acc.is_empty() {
        return Ok(());
    }
    let n = ticks as f64;
    let mut keep: Vec<&String> = Vec::new();
    let rankings: [fn(&MinuteAcc) -> f64; 4] = [|m| m.cpu, |m| m.mem, |m| m.disk, |m| m.gpu];
    for key in rankings {
        let mut v: Vec<(&String, f64)> = acc.iter().map(|(k, m)| (k, key(m))).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (k, val) in v.into_iter().take(TOP_PER_METRIC) {
            if val > 0.0 && !keep.contains(&k) {
                keep.push(k);
            }
        }
    }
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO process_samples(ts, name, cpu, mem, mem_max, disk, gpu) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for k in keep {
            let m = &acc[k];
            stmt.execute(params![ts, k, m.cpu / n, (m.mem / n) as i64, m.mem_max as i64, m.disk / n, m.gpu / n])?;
        }
    }
    tx.commit()
}
