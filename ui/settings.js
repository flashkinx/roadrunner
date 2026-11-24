// Einstellungen: speichert jede Aenderung sofort (wie die macOS-Systemeinstellungen).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const DAYS = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"];
const WEEKS_PER_MONTH = 365.25 / 12 / 7;
const fmt = new Intl.NumberFormat("de-AT");

let cfg = null;
let status = null;

// ---------- Hilfen ----------
function toast(msg, ok = false) {
  const t = $("toast");
  t.textContent = msg;
  t.className = ok ? "toast ok" : "toast";
  t.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (t.hidden = true), ok ? 1800 : 5000);
}

let saveTimer = null;
function save(delay = 350) {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try {
      cfg = await invoke("save_config", { cfg });
    } catch (e) {
      toast(String(e));
    }
  }, delay);
}

// Session-Token fuer die Places-Abrechnung (eine Sitzung = Tippen bis Auswahl)
function newSession() {
  if (crypto.randomUUID) return crypto.randomUUID();
  const b = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

function hm(s) {
  const [h, m] = (s || "0:0").split(":").map(Number);
  return h * 60 + m;
}

function setSeg(el, value) {
  el.querySelectorAll("button").forEach((b) => b.classList.toggle("on", b.dataset.v === value));
}

function onSeg(el, fn) {
  el.addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    setSeg(el, b.dataset.v);
    fn(b.dataset.v);
  });
}

// ---------- Navigation ----------
document.querySelectorAll(".nav").forEach((a) =>
  a.addEventListener("click", () => {
    document.querySelectorAll(".nav").forEach((n) => n.classList.toggle("active", n === a));
    document.querySelectorAll(".page").forEach((p) => p.classList.toggle("active", p.id === `page-${a.dataset.page}`));
    $("page-title").textContent = a.textContent.trim().replace(/^.\s*/, "");
    if (a.dataset.page === "general") refreshKeyState();
    if (a.dataset.page === "route") refreshLocState();
  })
);
// Titel ohne Symbol
$("page-title").textContent = "Übersicht";

// ---------- Übersicht ----------
function renderStatus(s) {
  status = s;
  const hero = $("hero");
  hero.dataset.color = s.color;
  hero.dataset.kind = s.kind;
  $("h-min").textContent = s.minutes ?? "–";

  if (s.origin && s.dest) {
    const how = s.manual ? "fix für heute" : s.fromLocation ? "vom aktuellen Standort" : "automatisch";
    $("h-route").textContent = `${s.origin} → ${s.dest} (${how})`;
  } else {
    $("h-route").textContent = s.message || "Noch kein Wert";
  }

  const meta = [];
  if (s.minutes != null) {
    meta.push(s.delayMin > 0 ? `+${s.delayMin} min durch Verkehr` : "kein Stau");
    if (s.distanceKm != null) meta.push(`${s.distanceKm.toLocaleString("de-AT")} km`);
  }
  if (s.updatedAt) meta.push(`Stand ${s.updatedAt}`);
  if (s.message && s.origin) meta.push(s.message);
  $("h-meta").textContent = meta.join(" · ");

  $("usage").textContent = `${fmt.format(s.usageCount)} (Freikontingent ${fmt.format(s.freeQuota)})`;
  renderPreview();
}

$("btn-refresh").addEventListener("click", () => invoke("refresh_now"));
onSeg($("seg-dir"), (v) => invoke("set_direction", { mode: v }));

function renderDir() {
  const today = new Date().toLocaleDateString("sv-SE"); // YYYY-MM-DD
  const m = cfg.manual && cfg.manual.date === today ? cfg.manual.direction : "auto";
  setSeg($("seg-dir"), m);
  $("seg-work").textContent = `Nach ${cfg.workName}`;
  $("seg-home").textContent = `Nach ${cfg.homeName}`;
}

// ---------- Route ----------
function renderPlace(which) {
  const p = cfg[which];
  $(`${which}-addr`).value = p ? p.label : "";
  $(`${which}-coords`).textContent = p ? `${p.lat.toFixed(5)}, ${p.lng.toFixed(5)}` : "Noch nicht gesetzt";
  $(`${which}-name`).value = cfg[`${which}Name`];
}

function setupAutocomplete(which) {
  const input = $(`${which}-addr`);
  const list = input.parentElement.querySelector(".ac-list");
  let session = newSession();
  let items = [];
  let hl = -1;
  let timer = null;
  let seq = 0;

  const close = () => { list.hidden = true; items = []; hl = -1; };
  const draw = () => {
    list.innerHTML = "";
    items.forEach((it, i) => {
      const li = document.createElement("li");
      if (it.msg) li.className = "msg";
      if (i === hl) li.classList.add("hl");
      li.textContent = it.main;
      if (it.secondary) {
        const s = document.createElement("small");
        s.textContent = it.secondary;
        li.appendChild(s);
      }
      li.addEventListener("mousedown", (e) => { e.preventDefault(); choose(i); });
      list.appendChild(li);
    });
    list.hidden = items.length === 0;
  };

  async function choose(i) {
    const it = items[i];
    if (!it || it.msg) return;
    close();
    try {
      const place = it.place || (await invoke("place_details", { placeId: it.placeId, session }));
      session = newSession();
      cfg[which] = place;
      renderPlace(which);
      input.blur();
      save(0);
    } catch (e) {
      toast(String(e));
    }
  }

  input.addEventListener("input", () => {
    clearTimeout(timer);
    const q = input.value.trim();
    timer = setTimeout(async () => {
      const my = ++seq;
      const c = await invoke("parse_coordinates", { text: q });
      if (my !== seq) return;
      if (c) {
        items = [{ main: `Koordinaten übernehmen`, secondary: c.label, place: c }];
        hl = 0;
        return draw();
      }
      if (q.length < 3) return close();
      try {
        const res = await invoke("autocomplete", { input: q, session });
        if (my !== seq) return;
        items = res.length ? res : [{ main: "Keine Treffer", msg: true }];
        hl = res.length ? 0 : -1;
      } catch (e) {
        items = [{ main: String(e), msg: true }];
        hl = -1;
      }
      draw();
    }, 250);
  });

  input.addEventListener("keydown", (e) => {
    if (list.hidden) return;
    if (e.key === "ArrowDown") { hl = Math.min(items.length - 1, hl + 1); draw(); e.preventDefault(); }
    else if (e.key === "ArrowUp") { hl = Math.max(0, hl - 1); draw(); e.preventDefault(); }
    else if (e.key === "Enter") { choose(hl); e.preventDefault(); }
    else if (e.key === "Escape") { close(); renderPlace(which); }
  });
  input.addEventListener("blur", () => setTimeout(() => { close(); renderPlace(which); }, 120));

  $(`${which}-name`).addEventListener("change", (e) => {
    cfg[`${which}Name`] = e.target.value.trim() || (which === "home" ? "Zuhause" : "Firma");
    renderDir();
    save();
  });
}
setupAutocomplete("home");
setupAutocomplete("work");

document.querySelectorAll("[data-here]").forEach((b) =>
  b.addEventListener("click", async () => {
    const which = b.dataset.here;
    const p = await invoke("current_location");
    if (!p) {
      toast("Kein Standort verfügbar. Ortungsdienste für Roadrunner in den Systemeinstellungen erlauben und kurz warten.");
      return;
    }
    cfg[which] = { ...p, label: `Standort ${p.label}` };
    renderPlace(which);
    save(0);
  })
);

async function refreshLocState() {
  const el = $("loc-state");
  if (!cfg.useLocation) {
    el.textContent = "Aus: Fahrzeit immer zwischen den beiden Adressen";
    return;
  }
  const p = await invoke("location_permission");
  el.textContent = {
    granted: "An: Richtung und Start nach aktuellem Standort",
    notDetermined: "Wartet auf Freigabe durch macOS …",
    denied: "Verweigert: in Systemeinstellungen › Datenschutz › Ortungsdienste erlauben",
    off: "Wird gestartet …",
  }[p];
}

$("use-location").addEventListener("change", (e) => {
  cfg.useLocation = e.target.checked;
  save(0);
  setTimeout(refreshLocState, 800);
});
$("switch-time").addEventListener("change", (e) => { if (e.target.value) { cfg.switchTime = e.target.value; save(); } });
$("radius").addEventListener("change", (e) => { cfg.radiusM = Math.max(50, +e.target.value || 500); save(); });

// ---------- Ampel ----------
function activeT() { return cfg.basis === "total" ? cfg.total : cfg.delay; }

function renderAmpel() {
  setSeg($("seg-basis"), cfg.basis);
  const t = activeT();
  $("t-green").value = t.greenMax;
  $("t-yellow").value = t.yellowMax;
  $("t-red").textContent = `ab ${t.yellowMax + 1} min`;
  $("basis-hint").textContent = cfg.basis === "total"
    ? "Farbe nach der gesamten Fahrzeit mit aktuellem Verkehr."
    : "Farbe nach der Verzögerung gegenüber der Fahrzeit ohne Verkehr. Die Pille zeigt trotzdem die Gesamtminuten.";

  const max = Math.max(t.yellowMax * 1.3, t.yellowMax + 5);
  const pct = (v) => Math.max(0, Math.min(100, (v / max) * 100));
  const bar = $("scale").querySelector(".bar");
  bar.querySelector(".seg-g").style.width = `${pct(t.greenMax)}%`;
  bar.querySelector(".seg-y").style.width = `${pct(t.yellowMax) - pct(t.greenMax)}%`;
  $("ticks").innerHTML = "";
  for (const v of [0, t.greenMax, t.yellowMax]) {
    const s = document.createElement("span");
    s.style.left = `${pct(v)}%`;
    s.textContent = v;
    $("ticks").appendChild(s);
  }
  renderPreview();
}

function renderPreview() {
  if (!cfg || !status) return;
  const t = activeT();
  const v = cfg.basis === "total" ? status.minutes : status.delayMin;
  const marker = $("marker");
  if (v == null) {
    marker.style.display = "none";
    $("preview").textContent = "";
    return;
  }
  const max = Math.max(t.yellowMax * 1.3, t.yellowMax + 5);
  marker.style.display = "block";
  marker.style.left = `${Math.max(0, Math.min(100, (v / max) * 100))}%`;
  const col = v <= t.greenMax ? "Grün" : v <= t.yellowMax ? "Gelb" : "Rot";
  const what = cfg.basis === "total" ? `${v} min Fahrzeit` : `${v} min Verzögerung`;
  $("preview").textContent = `Aktuell ${what} → ${col}`;
}

onSeg($("seg-basis"), (v) => { cfg.basis = v; renderAmpel(); save(0); });

function onThreshold() {
  const t = activeT();
  let g = Math.max(0, Math.round(+$("t-green").value || 0));
  let y = Math.max(0, Math.round(+$("t-yellow").value || 0));
  if (y < g) y = g;
  t.greenMax = g;
  t.yellowMax = y;
  renderAmpel();
  save();
}
$("t-green").addEventListener("change", onThreshold);
$("t-yellow").addEventListener("change", onThreshold);

// ---------- Aktualisierung ----------
function buildSchedule() {
  const box = $("schedule");
  box.innerHTML = "";
  cfg.schedule.forEach((w, i) => {
    const row = document.createElement("div");
    row.className = "row day" + (w.enabled ? "" : " off");
    row.innerHTML = `
      <label>${DAYS[i]}</label>
      <input type="time" class="text time" value="${w.start}">
      <span class="dash">bis</span>
      <input type="time" class="text time" value="${w.end}">
      <input type="checkbox" class="switch">`;
    const [start, end] = row.querySelectorAll("input[type=time]");
    const on = row.querySelector(".switch");
    on.checked = w.enabled;
    on.addEventListener("change", () => { w.enabled = on.checked; row.classList.toggle("off", !w.enabled); changed(); });
    start.addEventListener("change", () => { if (start.value) { w.start = start.value; changed(); } });
    end.addEventListener("change", () => { if (end.value) { w.end = end.value; changed(); } });
    box.appendChild(row);
  });
  box.classList.toggle("disabled", cfg.alwaysActive);
  box.style.opacity = cfg.alwaysActive ? 0.45 : 1;
  box.style.pointerEvents = cfg.alwaysActive ? "none" : "";
  function changed() { renderProjection(); save(); }
}

function renderProjection() {
  let perWeek = 0;
  if (cfg.alwaysActive) perWeek = (7 * 1440) / cfg.intervalMin;
  else
    for (const w of cfg.schedule) {
      if (!w.enabled) continue;
      let span = hm(w.end) - hm(w.start);
      if (span <= 0) span += 1440;
      perWeek += Math.floor(span / cfg.intervalMin);
    }
  const month = Math.round(perWeek * WEEKS_PER_MONTH);
  const el = $("projection");
  const share = cfg.freeQuota > 0 ? month / cfg.freeQuota : Infinity;
  el.textContent = `rund ${fmt.format(month)} Abfragen` + (cfg.freeQuota > 0 ? ` (${Math.round(share * 100)} % des Freikontingents)` : "");
  el.style.color = share > 1 ? "var(--red)" : share > 0.8 ? "#ff9f0a" : "";
}

$("interval").addEventListener("change", (e) => {
  cfg.intervalMin = Math.min(120, Math.max(1, Math.round(+e.target.value || 5)));
  e.target.value = cfg.intervalMin;
  renderProjection();
  save();
});
$("always-active").addEventListener("change", (e) => { cfg.alwaysActive = e.target.checked; buildSchedule(); renderProjection(); save(0); });
$("quota").addEventListener("change", (e) => { cfg.freeQuota = Math.max(0, Math.round(+e.target.value || 0)); renderProjection(); save(); });

// ---------- Allgemein ----------
$("show-mini").addEventListener("change", (e) => { cfg.showMini = e.target.checked; save(0); });
$("show-duration").addEventListener("change", (e) => { cfg.showDuration = e.target.checked; save(0); });
$("show-arrival").addEventListener("change", (e) => { cfg.showArrival = e.target.checked; save(0); });
$("pill-opacity").addEventListener("input", (e) => {
  cfg.pillOpacity = +e.target.value / 100;
  $("pill-opacity-val").textContent = `${e.target.value} %`;
  save(120);
});
onSeg($("seg-appearance"), (v) => { cfg.appearance = v; save(0); });
$("pill-scale").addEventListener("input", (e) => {
  cfg.pillScale = +e.target.value / 100;
  $("pill-scale-val").textContent = `${e.target.value} %`;
  save(120);
});
$("on-top").addEventListener("change", (e) => { cfg.alwaysOnTop = e.target.checked; save(0); });
$("autostart").addEventListener("change", (e) => { cfg.autostart = e.target.checked; save(0); });

async function refreshKeyState() {
  const has = await invoke("has_api_key");
  $("key-state").textContent = has ? "Im Schlüsselbund hinterlegt." : "Noch kein Key hinterlegt.";
}
$("btn-key").addEventListener("click", async () => {
  try {
    await invoke("set_api_key", { key: $("api-key").value });
    $("api-key").value = "";
    refreshKeyState();
    toast("API-Key gesichert", true);
  } catch (e) {
    toast(String(e));
  }
});

// ---------- Laden ----------
function renderAll() {
  renderPlace("home");
  renderPlace("work");
  renderDir();
  $("use-location").checked = cfg.useLocation;
  $("switch-time").value = cfg.switchTime;
  $("radius").value = cfg.radiusM;
  renderAmpel();
  $("interval").value = cfg.intervalMin;
  $("always-active").checked = cfg.alwaysActive;
  $("quota").value = cfg.freeQuota;
  buildSchedule();
  renderProjection();
  $("show-mini").checked = cfg.showMini;
  $("show-duration").checked = cfg.showDuration !== false;
  $("show-arrival").checked = !!cfg.showArrival;
  $("pill-scale").value = Math.round((cfg.pillScale || 1) * 100);
  $("pill-scale-val").textContent = `${$("pill-scale").value} %`;
  $("pill-opacity").value = Math.round((cfg.pillOpacity || 1) * 100);
  $("pill-opacity-val").textContent = `${$("pill-opacity").value} %`;
  setSeg($("seg-appearance"), cfg.appearance || "system");
  $("on-top").checked = cfg.alwaysOnTop;
  $("autostart").checked = cfg.autostart;
}

(async () => {
  cfg = await invoke("get_config");
  renderAll();
  renderStatus(await invoke("get_status"));
  refreshKeyState();
  refreshLocState();
  // Vorgegebene Startseite (ROADRUNNER_PAGE, fuer Screenshots)
  const sp = await invoke("start_page");
  if (sp) {
    const page = sp === "guide" ? "general" : sp;
    document.querySelector(`[data-page="${page}"]`)?.click();
    if (sp === "guide") {
      $("guide").open = true;
      setTimeout(() => $("guide").scrollIntoView({ block: "start" }), 50);
    }
    return;
  }
  // Erststart: direkt zur fehlenden Einstellung
  if (!(await invoke("has_api_key"))) document.querySelector('[data-page="general"]').click();
  else if (!cfg.home || !cfg.work) document.querySelector('[data-page="route"]').click();
})();

listen("status", (e) => renderStatus(e.payload));
listen("config", (e) => {
  // Aenderungen aus dem Menue (Richtung, Mini, Vordergrund) uebernehmen, ohne laufende Eingaben zu stoeren
  const c = e.payload;
  if (!cfg) return;
  cfg.manual = c.manual;
  cfg.showMini = c.showMini;
  cfg.alwaysOnTop = c.alwaysOnTop;
  cfg.usage = c.usage;
  $("show-mini").checked = c.showMini;
  $("on-top").checked = c.alwaysOnTop;
  cfg.showDuration = c.showDuration;
  cfg.showArrival = c.showArrival;
  $("show-duration").checked = c.showDuration;
  $("show-arrival").checked = c.showArrival;
  renderDir();
});

// ---------- Anleitung ----------
document.querySelectorAll(".go[data-url]").forEach((a) =>
  a.addEventListener("click", async () => {
    try {
      await invoke("open_url", { url: a.dataset.url });
    } catch (e) {
      toast(String(e));
    }
  })
);
$("btn-copy-prompt").addEventListener("click", async () => {
  try {
    await invoke("copy_text", { text: $("claude-prompt").textContent });
    toast("Auftrag in die Zwischenablage kopiert", true);
  } catch (e) {
    toast(String(e));
  }
});
// Ohne Key die Anleitung gleich aufgeklappt zeigen
invoke("has_api_key").then((has) => { if (!has) $("guide").open = true; });
