import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setLang, getLang, t } from "./i18n.js";
import * as f from "./format.js";
import { makeChart, sparkline, cssVar } from "./chart.js";
import defaultIcon from "./default-app-icon.webp";

const RANGES = [900, 3600, 86400, 604800, 2592000];
const LIVE_MS = 5000;
const HISTORY_MS = 30000;
const WARN_KINDS = new Set(["cpu_high", "cpu_hot", "gpu_hot"]);
const IS_WINDOWS = navigator.userAgent.includes("Windows");

const state = {
  page: "overview",
  range: 3600,
  custom: null, // { from, to } when a custom range or an insight period is shown
  chartTab: "cpu",
  sortBy: "cpu",
  sel: null, // { from, to } dragged on the chart
  series: null,
  insights: [],
  top: [],
  apps: [],
  latest: null,
  settings: null,
  selectedApp: null,
  plots: {},
};

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

function call(cmd, args) {
  return invoke(cmd, args).catch((e) => {
    console.error(cmd, e);
    return null;
  });
}

function currentWindow() {
  if (state.custom) return state.custom;
  const to = f.now();
  return { from: to - state.range, to };
}

function rangeText() {
  if (state.custom) return t("customRange")(f.time(state.custom.from, true), f.time(state.custom.to, true));
  return t("rangeLabel")[state.range];
}

// ---------- theme ----------
function applyTheme(choice) {
  let theme = choice;
  if (choice === "system") theme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  document.documentElement.dataset.theme = theme;
}
function savedTheme() {
  try {
    return localStorage.getItem("theme") || "dark";
  } catch {
    return "dark";
  }
}

// ---------- KPI cards ----------
function seriesStats(values, t) {
  let sum = 0, n = 0, max = -Infinity, maxAt = null;
  values.forEach((v, i) => {
    if (v == null) return;
    sum += v;
    n++;
    if (v > max) {
      max = v;
      maxAt = t[i];
    }
  });
  return n ? { avg: sum / n, max, maxAt } : null;
}

function renderKpis() {
  const s = state.latest;
  const ser = state.series;
  const sum = (a, b) => a.map((v, i) => (v == null && b[i] == null ? null : (v ?? 0) + (b[i] ?? 0)));
  const accent = cssVar("--accent");
  const orange = cssVar("--orange");
  const cpuStats = ser ? seriesStats(ser.cpu, ser.t) : null;
  const temp = s?.cpu_temp ?? s?.gpu_temp ?? null;
  const memPct = s ? (s.mem_used / s.mem_total) * 100 : null;

  const cards = [
    {
      id: "cpu", tab: "cpu", label: "CPU",
      value: s ? f.pct(s.cpu) : "–",
      sub: cpuStats ? t("kpiCpuSub")(f.pct(cpuStats.avg), f.pct(cpuStats.max), f.time(cpuStats.maxAt)) : "",
      spark: ser ? sparkline(ser.cpu, accent, 100) : "",
      flag: s && s.cpu >= 80 ? t("high") : "",
    },
    {
      id: "mem", tab: "cpu", label: "RAM",
      value: s ? f.pct(memPct) : "–",
      sub: s ? t("kpiMemSub")(f.bytes(s.mem_used), f.bytes(s.mem_total)) : "",
      spark: ser ? sparkline(ser.mem, orange, 100) : "",
      flag: memPct >= 85 ? t("high") : "",
    },
    {
      id: "disk", tab: "disk", label: t("disk"),
      value: s ? f.rate(s.disk_read + s.disk_write) : "–",
      sub: s ? t("kpiDiskSub")(f.pct(s.disk_busy)) : "",
      spark: ser ? sparkline(sum(ser.disk_read, ser.disk_write), accent) : "",
    },
    {
      id: "gpu", tab: "gpu", label: "GPU",
      value: s?.gpu != null ? f.pct(s.gpu) : "–",
      sub: s ? t("kpiGpuSub")(s.gpu_mem ? f.bytes(s.gpu_mem) : "") : "",
      spark: ser ? sparkline(ser.gpu, accent, 100) : "",
    },
    {
      id: "net", tab: "net", label: t("net"),
      value: s ? f.rate(s.net_rx) : "–",
      sub: s ? t("kpiNetSub")(f.rate(s.net_tx)) : "",
      spark: ser ? sparkline(ser.net_rx, accent) : "",
    },
    {
      id: "temp", tab: "temp", label: t("temp"),
      value: f.celsius(temp),
      sub: temp == null ? t("kpiTempMissing") : t("kpiTempSub")(s.cpu_temp != null && s.gpu_temp != null ? f.celsius(s.gpu_temp) : ""),
      spark: ser ? sparkline(ser.cpu_temp, orange) : "",
      flag: temp >= 90 ? t("hot") : "",
    },
  ];

  $("kpis").innerHTML = cards
    .map(
      (c) => `<button class="kpi${c.tab === state.chartTab && (c.id !== "mem" || state.chartTab !== "cpu") ? " active" : ""}" data-tab="${c.tab}">
        <div class="kpi-top"><span class="kpi-label">${esc(c.label)}</span>${c.flag ? `<span class="flag">${esc(c.flag)}</span>` : ""}</div>
        <div class="kpi-value">${esc(c.value)}</div>
        <div class="kpi-sub">${esc(c.sub)}</div>
        ${c.spark}
      </button>`,
    )
    .join("");
  $("kpis").querySelectorAll(".kpi").forEach((el) => el.addEventListener("click", () => selectTab(el.dataset.tab)));
}

// ---------- insights ----------
const ICONS = {
  warn: "M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
  mem: "M6 19v-3M10 19v-3M14 19v-3M18 19v-3M4 7h16v9H4z",
  disk: "M3 12h18M3 12a9 3 0 0 0 18 0M3 12V7a9 3 0 0 1 18 0v5M3 12v5a9 3 0 0 0 18 0v-5",
  temp: "M14 14.8V4a2 2 0 0 0-4 0v10.8a4 4 0 1 0 4 0z",
};
const KIND_ICON = { cpu_high: "warn", mem_high: "mem", disk_busy: "disk", cpu_hot: "temp", gpu_hot: "temp" };
const APP_FMT = { cpu_high: f.pct, mem_high: f.bytes, disk_busy: f.rate, cpu_hot: f.pct, gpu_hot: f.pct };
const PEAK_FMT = { cpu_high: f.pct, mem_high: f.pct, disk_busy: f.pct, cpu_hot: f.celsius, gpu_hot: f.celsius };

function insightHtml(ins, i) {
  const [title, body] = t("insight")[ins.kind](
    f.duration(ins.to - ins.from),
    PEAK_FMT[ins.kind](ins.peak),
    ins.app ? esc(ins.app) : null,
    ins.app_value != null ? APP_FMT[ins.kind](ins.app_value) : "",
  );
  const when = ins.to - ins.from >= 60 ? `${f.time(ins.from, true)} – ${f.time(ins.to)}` : f.time(ins.from, true);
  return `<div class="insight${WARN_KINDS.has(ins.kind) ? " warn" : ""}">
    <span class="insight-icon"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="${ICONS[KIND_ICON[ins.kind]]}"/></svg></span>
    <div class="insight-body"><div><strong>${title}</strong> <span class="muted">${body}</span></div><div class="mono muted">${when}</div></div>
    <button class="link" data-insight="${i}">${esc(t("viewRange"))}</button>
  </div>`;
}

function renderInsightList(el, list) {
  if (!list.length) {
    el.innerHTML = `<div class="all-good"><span class="dot"></span>${esc(t("allGood"))}</div>`;
    return;
  }
  el.innerHTML = list.map((ins) => insightHtml(ins, state.insights.indexOf(ins))).join("");
  el.querySelectorAll("[data-insight]").forEach((b) => b.addEventListener("click", () => focusInsight(state.insights[+b.dataset.insight])));
}

function renderInsights() {
  renderInsightList($("overview-insights"), state.insights.slice(0, 3));
  renderInsightList($("all-insights"), state.insights);
  const warn = state.insights.filter((i) => WARN_KINDS.has(i.kind)).length;
  $("insight-count").hidden = warn === 0;
  $("insight-count").textContent = warn;
}

/** Show an insight's period: zoom the range around it and select it on the chart. */
function focusInsight(ins) {
  const len = ins.to - ins.from;
  const pad = Math.max(len, 600);
  state.custom = { from: ins.from - pad, to: Math.min(ins.to + pad, f.now()) };
  state.sel = { from: ins.from, to: ins.to };
  state.chartTab = ins.kind.endsWith("_hot") ? "temp" : ins.kind === "disk_busy" ? "disk" : "cpu";
  state.sortBy = { cpu_high: "cpu", mem_high: "mem", disk_busy: "disk", cpu_hot: "cpu", gpu_hot: "gpu" }[ins.kind];
  location.hash = "#overview";
  renderRanges();
  loadHistory();
}

// ---------- chart ----------
function tabConfig(tab) {
  const s = state.series;
  const S = t("series");
  const accent = cssVar("--accent");
  const orange = cssVar("--orange");
  const soft = cssVar("--accent-soft");
  switch (tab) {
    case "disk":
      return { fmt: f.rate, series: [{ label: S.read, values: s.disk_read, color: accent, fill: soft }, { label: S.write, values: s.disk_write, color: orange }] };
    case "gpu":
      return { fmt: f.pct, yMax: 100, series: [{ label: S.gpu, values: s.gpu, color: accent, fill: soft }] };
    case "net":
      return { fmt: f.rate, series: [{ label: S.down, values: s.net_rx, color: accent, fill: soft }, { label: S.up, values: s.net_tx, color: orange }] };
    case "temp":
      return {
        fmt: f.celsius,
        series: [
          { label: S.cpuTemp, values: s.cpu_temp, color: accent },
          { label: S.gpuTemp, values: s.gpu_temp, color: orange },
          { label: S.diskTemp, values: s.disk_temp, color: cssVar("--green") },
        ].filter((x) => x.values.some((v) => v != null)),
      };
    default:
      return { fmt: f.pct, yMax: 100, series: [{ label: S.cpu, values: s.cpu, color: accent, fill: soft }, { label: S.mem, values: s.mem, color: orange }] };
  }
}

function renderTabs() {
  $("chart-tabs").innerHTML = Object.entries(t("tabs"))
    .map(([k, label]) => `<button role="tab" aria-selected="${k === state.chartTab}" class="${k === state.chartTab ? "active" : ""}" data-tab="${k}">${esc(label)}</button>`)
    .join("");
  $("chart-tabs").querySelectorAll("button").forEach((b) => b.addEventListener("click", () => selectTab(b.dataset.tab)));
}

function selectTab(tab) {
  state.chartTab = tab;
  renderTabs();
  renderKpis();
  drawMainChart();
}

function drawMainChart() {
  state.plots.main?.destroy();
  state.plots.main = null;
  const el = $("chart");
  const s = state.series;
  const cfg = s && s.t.length ? tabConfig(state.chartTab) : null;
  const hasData = cfg && cfg.series.length && cfg.series.some((x) => x.values.some((v) => v != null));
  $("chart-empty").hidden = hasData;
  $("chart-empty").textContent = state.chartTab === "temp" ? t("noTemp") : t("noData");
  $("legend").innerHTML = hasData
    ? cfg.series.map((x) => `<span><i style="background:${x.color}"></i>${esc(x.label)}</span>`).join("") +
      (state.insights.length ? `<span><i class="ev"></i>${esc(t("events"))}</span>` : "")
    : "";
  if (!hasData) return;

  const bands = state.insights.map((i) => ({ from: i.from, to: i.to }));
  const extra = (idx) => {
    const ts = s.t[idx];
    const hit = state.insights.find((i) => ts >= i.from - s.bucket && ts <= i.to + s.bucket && i.app);
    return hit ? `${t("heaviest")}: ${hit.app} ${APP_FMT[hit.kind](hit.app_value)}` : "";
  };
  state.plots.main = makeChart(el, {
    t: s.t,
    series: cfg.series,
    fmt: cfg.fmt,
    yMax: cfg.yMax,
    bands,
    extra,
    onSelect: (sel) => {
      state.sel = sel;
      loadTop();
    },
  });
  if (state.sel) {
    const u = state.plots.main;
    const left = u.valToPos(state.sel.from, "x");
    const right = u.valToPos(state.sel.to, "x");
    u.setSelect({ left, width: Math.max(right - left, 2), top: 0, height: u.over.clientHeight }, false);
  }
}

// ---------- app icons ----------
const iconCache = new Map(); // name -> Promise<data URL | null>

/** Replaces a letter tile with the app's real icon once it has been extracted from its exe.
 *  Apps without an icon of their own (dwm, csrss, System...) get the generic Windows executable icon. */
function fillIcon(tile, name) {
  if (!tile || !name) return;
  if (!iconCache.has(name)) iconCache.set(name, call("get_app_icon", { name }));
  iconCache.get(name).then((url) => {
    const img = document.createElement("img");
    img.src = url || defaultIcon;
    img.alt = "";
    tile.replaceChildren(img);
    tile.classList.add("has-icon");
  });
}

// ---------- app tables ----------
const COLS = [
  ["cpu", "CPU", f.pct],
  ["mem", "RAM", f.bytes],
  ["disk", () => t("disk"), f.rate],
  ["gpu", "GPU", f.pct],
];

function renderAppTable(table, rows, { onRow, selected } = {}) {
  const sorted = [...rows].sort((a, b) => b[state.sortBy] - a[state.sortBy]);
  const max = Math.max(...sorted.map((r) => r[state.sortBy]), 1e-9);
  const head = `<thead><tr><th>${esc(t("app"))}</th>${COLS.map(
    ([k, label]) => `<th data-sort="${k}" class="${k === state.sortBy ? "sorted" : ""}">${esc(typeof label === "function" ? label() : label)}${k === state.sortBy ? " ↓" : ""}</th>`,
  ).join("")}<th>${esc(t("present"))}</th></tr></thead>`;
  const body = sorted
    .map((r) => {
      const cells = COLS.map(([k, , fmt]) =>
        k === state.sortBy
          ? `<td><div class="bar-cell"><div class="bar"><div style="width:${Math.round((r[k] / max) * 100)}%"></div></div><span>${fmt(r[k])}</span></div></td>`
          : `<td>${fmt(r[k])}</td>`,
      ).join("");
      return `<tr data-name="${esc(r.name)}" class="${r.name === selected ? "selected" : ""}"><td><div class="app-cell"><span class="tile" style="background:${f.appColor(r.name)}">${esc(r.name[0]?.toUpperCase() ?? "?")}</span><span>${esc(r.name)}</span></div></td>${cells}<td class="muted">${t("minutes")(r.minutes)}</td></tr>`;
    })
    .join("");
  table.innerHTML = head + `<tbody>${body}</tbody>`;
  table.querySelectorAll("tbody tr").forEach((tr) => fillIcon(tr.querySelector(".tile"), tr.dataset.name));
  table.querySelectorAll("th[data-sort]").forEach((th) =>
    th.addEventListener("click", () => {
      state.sortBy = th.dataset.sort;
      loadTop();
      if (state.page === "apps") loadApps();
    }),
  );
  if (onRow) table.querySelectorAll("tbody tr").forEach((tr) => tr.addEventListener("click", () => onRow(tr.dataset.name)));
}

async function loadTop() {
  const { from, to } = state.sel ?? currentWindow();
  state.top = (await call("get_top_apps", { from, to, metric: state.sortBy })) ?? [];
  $("top-h").innerHTML = `${esc(t("topTitle"))} <span class="muted">· ${esc(state.sel ? t("customRange")(f.time(state.sel.from, true), f.time(state.sel.to, true)) : rangeText())}</span>`;
  $("clear-sel").hidden = !state.sel;
  $("top-empty").hidden = state.top.length > 0;
  renderAppTable($("top-table"), state.top, { onRow: openApp });
}

// ---------- apps page ----------
async function loadApps() {
  const { from, to } = currentWindow();
  state.apps = (await call("get_top_apps", { from, to, metric: state.sortBy })) ?? [];
  $("apps-h").textContent = t("appsTitle")(rangeText());
  $("apps-empty").hidden = state.apps.length > 0;
  renderAppTable($("apps-table"), state.apps, { onRow: openApp, selected: state.selectedApp });
  if (state.selectedApp) loadAppDetail();
}

function openApp(name) {
  state.selectedApp = name;
  if (state.page !== "apps") location.hash = "#apps";
  else loadApps();
}

async function loadAppDetail() {
  const name = state.selectedApp;
  const { from, to } = currentWindow();
  const h = await call("get_app_history", { name, from, to });
  $("app-detail").hidden = false;
  $("app-name").textContent = name;
  $("app-tile").textContent = name[0]?.toUpperCase() ?? "?";
  $("app-tile").classList.remove("has-icon");
  $("app-tile").style.background = f.appColor(name);
  fillIcon($("app-tile"), name);
  state.plots.appCpu?.destroy();
  state.plots.appMem?.destroy();
  if (!h || !h.t.length) {
    $("app-stats").innerHTML = "";
    return;
  }
  const avg = (a) => a.reduce((x, y) => x + y, 0) / a.length;
  const S = t("appStats");
  const stats = [
    [S.cpuAvg, f.pct(avg(h.cpu))],
    [S.cpuMax, f.pct(Math.max(...h.cpu))],
    [S.memAvg, f.bytes(avg(h.mem))],
    [S.memMax, f.bytes(Math.max(...h.mem))],
    [S.minutes, t("minutes")(h.t.length)],
  ];
  $("app-stats").innerHTML = stats.map(([k, v]) => `<div class="stat"><div>${esc(k)}</div><div>${esc(v)}</div></div>`).join("");
  state.plots.appCpu = makeChart($("app-chart-cpu"), {
    t: h.t, height: 180, fmt: f.pct,
    series: [{ label: "CPU", values: h.cpu, color: cssVar("--accent"), fill: cssVar("--accent-soft") }],
  });
  state.plots.appMem = makeChart($("app-chart-mem"), {
    t: h.t, height: 180, fmt: f.bytes,
    series: [{ label: "RAM", values: h.mem, color: cssVar("--orange") }],
  });
}

// ---------- live status ----------
async function loadLatest() {
  state.latest = await call("get_latest");
  renderKpis();
  renderRecorder();
  renderTempStatus();
}

function renderRecorder() {
  const st = state.settings;
  const s = state.latest;
  const paused = st?.paused;
  $("rec-dot").classList.toggle("paused", !!paused);
  $("rec-label").textContent = paused ? t("paused") : t("recording");
  $("rec-self").textContent = s ? t("selfUsage")(f.bytes(s.self_mem), f.pct(s.self_cpu)) : "";
  if (st) {
    const days = st.first_ts ? Math.floor((f.now() - st.first_ts) / 86400) : 0;
    $("rec-history").textContent = t("historyInfo")(days >= 1 ? t("days")(days) : t("lessThanDay"), f.bytes(st.db_bytes));
  }
}

function renderTempStatus() {
  const s = state.latest;
  const ok = s && (s.cpu_temp != null || s.gpu_temp != null || s.disk_temp != null);
  $("temp-status").textContent = ok ? t("tempStatusOk")(s.temp_source) : t("tempStatusNone");
  // The help text describes the Windows installer; other platforms read kernel sensors.
  if (!IS_WINDOWS) document.querySelector('[data-i18n="tempHelp"]').textContent = t("tempHelpOther");
}

// ---------- settings ----------
async function loadSettings() {
  state.settings = await call("get_settings");
  return state.settings;
}

async function renderSettings() {
  const st = await loadSettings();
  if (!st) return;
  $("opt-autostart").checked = st.autostart;
  $("opt-paused").checked = st.paused;
  $("opt-retention").value = String(st.retention_days);
  $("opt-lang").value = getLang();
  $("opt-theme").value = savedTheme();
  $("opt-info").textContent = t("dataInfo")(f.bytes(st.db_bytes), st.first_ts ? f.time(st.first_ts, true) : "");
  renderTempStatus();
}

function bindSettings() {
  $("opt-autostart").addEventListener("change", (e) => call("set_autostart", { enabled: e.target.checked }));
  $("opt-paused").addEventListener("change", async (e) => {
    await call("set_paused", { paused: e.target.checked });
    await loadSettings();
    renderRecorder();
  });
  $("opt-retention").addEventListener("change", (e) => call("set_retention_days", { days: Number(e.target.value) }));
  $("opt-lang").addEventListener("change", async (e) => {
    await call("set_language", { language: e.target.value });
    setLang(e.target.value);
    renderAll();
  });
  $("opt-theme").addEventListener("change", (e) => {
    try {
      localStorage.setItem("theme", e.target.value);
    } catch {}
    applyTheme(e.target.value);
    renderAll();
  });
}

// ---------- ranges & routing ----------
function renderRanges() {
  $("ranges").innerHTML = RANGES.map(
    (r) => `<button data-range="${r}" class="${!state.custom && r === state.range ? "active" : ""}">${esc(t("ranges")[r])}</button>`,
  ).join("");
  $("ranges").querySelectorAll("button").forEach((b) =>
    b.addEventListener("click", () => {
      state.range = +b.dataset.range;
      state.custom = null;
      state.sel = null;
      renderRanges();
      loadHistory();
    }),
  );
  $("custom-btn").classList.toggle("active", !!state.custom);
}

function bindCustomRange() {
  const btn = $("custom-btn");
  const form = $("custom-form");
  btn.addEventListener("click", () => {
    const { from, to } = currentWindow();
    $("custom-from").value = f.toLocalInput(from);
    $("custom-to").value = f.toLocalInput(to);
    form.hidden = !form.hidden;
    btn.setAttribute("aria-expanded", String(!form.hidden));
  });
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const from = f.fromLocalInput($("custom-from").value);
    const to = f.fromLocalInput($("custom-to").value);
    if (!(to > from)) return;
    state.custom = { from, to };
    state.sel = null;
    form.hidden = true;
    btn.setAttribute("aria-expanded", "false");
    renderRanges();
    loadHistory();
  });
  document.addEventListener("click", (e) => {
    if (!form.hidden && !e.target.closest(".custom-wrap")) form.hidden = true;
  });
}

const PAGE_TITLES = { overview: "navOverview", apps: "navApps", insights: "navInsights", settings: "navSettings" };

function route() {
  const page = (location.hash.slice(1) || "overview").split("?")[0];
  state.page = PAGE_TITLES[page] ? page : "overview";
  document.querySelectorAll(".nav-item").forEach((a) => a.classList.toggle("active", a.dataset.page === state.page));
  document.querySelectorAll(".page").forEach((p) => (p.hidden = p.id !== `page-${state.page}`));
  $("range-bar").hidden = state.page === "settings";
  renderHead();
  if (state.page === "apps") loadApps();
  if (state.page === "settings") renderSettings();
  if (state.page === "overview") drawMainChart();
}

function renderHead() {
  $("page-title").textContent = t(PAGE_TITLES[state.page]);
  $("page-sub").textContent = state.page === "settings" ? "" : state.custom ? rangeText() : t("updated")(f.longDate(f.now()));
}

async function loadHistory() {
  const { from, to } = currentWindow();
  const [series, insights] = await Promise.all([call("get_series", { from, to }), call("get_insights", { from, to })]);
  state.series = series;
  state.insights = insights ?? [];
  renderHead();
  renderKpis();
  renderInsights();
  renderTabs();
  if (state.page === "overview") drawMainChart();
  await loadTop();
  if (state.page === "apps") loadApps();
}

function renderAll() {
  setLang(getLang());
  renderRanges();
  renderHead();
  renderTabs();
  renderKpis();
  renderInsights();
  renderRecorder();
  drawMainChart();
  loadTop();
  if (state.page === "apps") loadApps();
  if (state.page === "settings") renderSettings();
}

// ---------- window controls ----------
function bindWindow() {
  const win = getCurrentWindow();
  $("win-min").addEventListener("click", () => win.minimize());
  $("win-max").addEventListener("click", () => win.toggleMaximize());
  $("win-close").addEventListener("click", () => win.close());
}

async function init() {
  applyTheme(savedTheme());
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (savedTheme() === "system") {
      applyTheme("system");
      renderAll();
    }
  });
  const st = await loadSettings();
  setLang(st?.language ?? (navigator.language?.toLowerCase().startsWith("vi") ? "vi" : "en"));
  bindWindow();
  bindSettings();
  bindCustomRange();
  $("clear-sel").addEventListener("click", () => {
    state.sel = null;
    state.plots.main?.setSelect({ left: 0, top: 0, width: 0, height: 0 }, false);
    loadTop();
  });
  $("app-close").addEventListener("click", () => {
    state.selectedApp = null;
    $("app-detail").hidden = true;
    loadApps();
  });
  window.addEventListener("hashchange", route);
  let resizeTimer;
  window.addEventListener("resize", () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      for (const [k, p] of Object.entries(state.plots)) {
        if (!p) continue;
        const el = p.root.parentElement;
        p.setSize({ width: el.clientWidth, height: k === "main" ? 280 : 180 });
      }
    }, 100);
  });

  renderRanges();
  renderTabs();
  route();
  await Promise.all([loadLatest(), loadHistory()]);

  setInterval(loadLatest, LIVE_MS);
  setInterval(async () => {
    if (document.hidden) return;
    await loadSettings();
    renderRecorder();
    // Keep custom ranges and drag selections still; only live ranges roll forward.
    if (!state.custom && !state.sel) loadHistory();
  }, HISTORY_MS);
}

init();
