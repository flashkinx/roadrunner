// Ampel-Pille (eine Zeile; Ziel und Stand im Tooltip)
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const win = window.__TAURI__.window.getCurrentWindow();

const pill = document.getElementById("pill");
const minEl = document.getElementById("min");
const deltaEl = document.getElementById("delta");
const arrEl = document.getElementById("arr");
let last = null;

// Ankunft bei Abfahrt jetzt: laeuft auch zwischen zwei Abrufen mit
function renderArrival() {
  if (!last || last.minutes == null) { arrEl.textContent = "–:–"; return; }
  const t = new Date(Date.now() + last.minutes * 60000);
  arrEl.textContent = t.toLocaleTimeString("de-AT", { hour: "2-digit", minute: "2-digit" });
}
setInterval(renderArrival, 15000);

function render(s) {
  last = s;
  renderArrival();
  pill.dataset.color = s.color;
  pill.dataset.kind = s.kind;
  minEl.textContent = s.minutes ?? "–";
  deltaEl.textContent = s.minutes != null && s.delayMin > 0 ? `+${s.delayMin}` : "";

  const lines = [];
  if (s.origin && s.dest) {
    const how = s.manual ? " (fix für heute)" : s.fromLocation ? " (vom Standort)" : "";
    lines.push(`${s.origin} → ${s.dest}${how}`);
  }
  const facts = [];
  if (s.minutes != null) facts.push(`${s.minutes} min`);
  if (s.minutes != null) facts.push(`Ankunft ca. ${arrEl.textContent} bei Abfahrt jetzt`);
  if (s.delayMin > 0) facts.push(`+${s.delayMin} min Verkehr`);
  if (s.updatedAt) facts.push(`Stand ${s.updatedAt}`);
  if (facts.length) lines.push(facts.join(" · "));
  if (s.kind === "loading") lines.push("Wird geladen …");
  if (s.message) lines.push(s.message);
  lines.push("Doppelklick: Einstellungen · Rechtsklick: Menü");
  pill.title = lines.join("\n");
}

// Größe: das Fenster skaliert Rust, der Inhalt skaliert hier mit
function applyScale(cfg) {
  document.body.style.zoom = cfg.pillScale || 1;
  document.body.dataset.dur = cfg.showDuration === false ? "0" : "1";
  document.body.dataset.arr = cfg.showArrival ? "1" : "0";
}

// Ziehen erst ab 3 px Bewegung, damit der Doppelklick nicht vom Drag verschluckt wird
let down = null;
pill.addEventListener("mousedown", (e) => {
  if (e.button === 0) down = { x: e.screenX, y: e.screenY };
});
window.addEventListener("mousemove", (e) => {
  if (!down || !(e.buttons & 1)) { down = null; return; }
  if (Math.abs(e.screenX - down.x) + Math.abs(e.screenY - down.y) > 3) {
    down = null;
    win.startDragging();
  }
});
window.addEventListener("mouseup", () => { down = null; });
pill.addEventListener("dblclick", () => invoke("open_settings"));

pill.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  invoke("context_menu");
});

listen("status", (e) => render(e.payload));
listen("config", (e) => applyScale(e.payload));
invoke("get_status").then(render);
invoke("get_config").then(applyScale);
