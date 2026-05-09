// extras.jsx — companion components and helpers for tokenpet.
// Loaded after pet.jsx, before app.jsx. Exports everything to window so
// app.jsx can pull them out via destructure (Babel scripts don't share scope).

const { useState: _useState, useEffect: _useEffect, useRef: _useRef } = React;

// ── Achievements ───────────────────────────────────────────────────────────
// `check` runs against `state` after each tick / mutation. Stable, idempotent.
const ACHIEVEMENTS = [
  { id: "first_feed", glyph: "✦", title: "First Bite",
    desc: "Burned your first tokens",
    check: (s) => (s.lifetimeTokens || 0) >= 1 },
  { id: "first_ship", glyph: "★", title: "First Ship",
    desc: "Merged your first PR",
    check: (s) => (s.prsMerged || 0) >= 1 },
  { id: "tok_1m", glyph: "◆", title: "Million-Token Club",
    desc: "Burned 1M tokens lifetime",
    check: (s) => (s.lifetimeTokens || 0) >= 1_000_000 },
  { id: "tok_10m", glyph: "◈", title: "Ten Million",
    desc: "Burned 10M tokens lifetime",
    check: (s) => (s.lifetimeTokens || 0) >= 10_000_000 },
  { id: "tok_100m", glyph: "❖", title: "Hundred Mil",
    desc: "Burned 100M tokens. Touch grass.",
    check: (s) => (s.lifetimeTokens || 0) >= 100_000_000 },
  { id: "ship_10", glyph: "✺", title: "Shipping Streak",
    desc: "Merged 10 PRs",
    check: (s) => (s.prsMerged || 0) >= 10 },
  { id: "ship_50", glyph: "✸", title: "Velocity",
    desc: "Merged 50 PRs",
    check: (s) => (s.prsMerged || 0) >= 50 },
  { id: "stage_pup", glyph: "❀", title: "Growing Up",
    desc: "Reached pup stage",
    check: (s) => !!s.flags?.stagePup },
  { id: "stage_adult", glyph: "❁", title: "Adulthood",
    desc: "Reached adult stage",
    check: (s) => !!s.flags?.stageAdult },
  { id: "stage_sage", glyph: "✿", title: "Ascended",
    desc: "Reached sage form",
    check: (s) => !!s.flags?.stageSage },
  { id: "night_owl", glyph: "☾", title: "Night Owl",
    desc: "Burned tokens between midnight–5am",
    check: (s) => !!s.flags?.nightOwl },
  { id: "collector_3", glyph: "✜", title: "Trio",
    desc: "Met 3 different species",
    check: (s) => (s.flags?.metSpecies?.length || 0) >= 3 },
  { id: "collector_all", glyph: "✦✦", title: "Collector",
    desc: "Met all 6 species",
    check: (s) => (s.flags?.metSpecies?.length || 0) >= 6 },
  { id: "revived", glyph: "♺", title: "Second Chance",
    desc: "Revived a pet from the brink",
    check: (s) => !!s.flags?.revived },
];

// Returns { set: Set of all unlocked ids, newly: Achievement[] just unlocked }.
function computeUnlocks(state, prevSet) {
  const set = new Set(prevSet);
  const newly = [];
  for (const a of ACHIEVEMENTS) {
    if (set.has(a.id)) continue;
    if (a.check(state)) { set.add(a.id); newly.push(a); }
  }
  return { set, newly };
}

// ── Treats ─────────────────────────────────────────────────────────────────
// `fx` is the stat delta (clamped at apply time). Tokens count toward usage.
const TREATS = [
  { id: "espresso", label: "espresso",  glyph: "☕",
    desc: "+energy, mild happiness",
    fx: { energy: 30, happiness: 4, fed: -2, tokens: 0 } },
  { id: "sugar",    label: "sugar cube", glyph: "❅",
    desc: "happy now, crashy later",
    fx: { energy: 12, happiness: 18, fed: 4, tokens: 0 } },
  { id: "kibble",   label: "kibble",     glyph: "▪",
    desc: "regular meal, full belly",
    fx: { energy: 4, happiness: 4, fed: 22, tokens: 0 } },
  { id: "byte",     label: "raw byte",   glyph: "◇",
    desc: "0xff feast — small token sim",
    fx: { energy: 6, happiness: 8, fed: 12, tokens: 8_000 } },
  { id: "flop",     label: "flop chip",  glyph: "✦",
    desc: "rare matrix-multiply snack",
    fx: { energy: 18, happiness: 14, fed: 14, tokens: 30_000 } },
  { id: "tea",      label: "chamomile",  glyph: "❀",
    desc: "calm and content",
    fx: { energy: -4, happiness: 12, fed: 0, tokens: 0 } },
];

// ── Bell flash ─────────────────────────────────────────────────────────────
// Flashes the terminal border for ~500ms. Used on big events (ship/evolve/treat).
function bellFlash() {
  const term = document.querySelector(".terminal");
  if (!term) return;
  term.classList.remove("bell-flash");
  // Force reflow so the animation can replay if called again immediately.
  // eslint-disable-next-line no-unused-expressions
  void term.offsetWidth;
  term.classList.add("bell-flash");
  setTimeout(() => term.classList.remove("bell-flash"), 600);
}

// ── Sparkline ──────────────────────────────────────────────────────────────
// Renders a 7-day token history as braille-style block glyphs. Today is
// highlighted with the accent glow so the eye lands on "right now".
function Sparkline({ values }) {
  const v = (values || []).slice(-7);
  if (v.length === 0) return null;
  const max = Math.max(1, ...v);
  const blocks = "▁▂▃▄▅▆▇█";
  return (
    <span className="sparkline" title="last 7 days · token usage">
      {v.map((n, i) => {
        const idx = Math.min(blocks.length - 1, Math.max(0, Math.round((n / max) * (blocks.length - 1))));
        const isToday = i === v.length - 1;
        return (
          <span key={i} className={`spark-day ${isToday ? "today" : ""}`}>
            {blocks[idx]}
          </span>
        );
      })}
    </span>
  );
}

// ── Hint strip ─────────────────────────────────────────────────────────────
// Persistent low-key bar under the activity feed pointing at discoverable bits.
function HintStrip({ onHelp }) {
  return (
    <div className="hint-strip">
      <span className="hint-key">type <kbd>help</kbd> or hit <kbd>?</kbd> for commands</span>
      <span className="hint-key"><kbd>↑</kbd>/<kbd>↓</kbd> history</span>
      <span className="hint-key"><kbd>tab</kbd> autocomplete</span>
      <span className="hint-key" style={{marginLeft:"auto", cursor:"pointer", color:"var(--accent)"}} onClick={onHelp}>
        ▸ open help
      </span>
    </div>
  );
}

// ── Help palette ───────────────────────────────────────────────────────────
function HelpPalette({ onClose }) {
  _useEffect(() => {
    const onKey = (e) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  const rows = [
    ["feed [n]",       "burn n tokens (default 47k) · feeds the pet"],
    ["treat <id>",     "give a treat: espresso, sugar, kibble, byte, flop, tea"],
    ["ship",           "merge a PR · big happiness spike"],
    ["pet",            "show affection · small +happy"],
    ["sleep",          "nap · restore energy"],
    ["status",         "print current vitals"],
    ["rename <name>",  "change your pet's name"],
    ["reroll [seed]",  "generate a new pet"],
    ["achievements",   "list unlocked + locked achievements"],
    ["graveyard",      "list past pets"],
    ["revive",         "bring your pet back from the dead"],
    ["clear",          "clear the activity feed"],
    ["—", "—"],
    ["↑ / ↓",          "command history"],
    ["tab",            "autocomplete"],
    ["?",              "open this palette"],
    ["esc",            "close this palette"],
  ];
  return (
    <div className="help-palette" onClick={onClose}>
      <div className="help-card" onClick={(e) => e.stopPropagation()}>
        <div className="help-title">─ glorp · command reference ─</div>
        <div className="help-grid">
          {rows.map(([k, d], i) => (
            <React.Fragment key={i}>
              <span className="hk">{k}</span>
              <span className="hd">{d}</span>
            </React.Fragment>
          ))}
        </div>
        <div className="help-close">click anywhere · or <kbd>esc</kbd> to close</div>
      </div>
    </div>
  );
}

// ── Toast stack ────────────────────────────────────────────────────────────
function ToastStack({ toasts }) {
  if (!toasts || toasts.length === 0) return null;
  return (
    <div className="toast-stack">
      {toasts.map((t) => (
        <div key={t.id} className={`toast ${t.exit ? "exit" : ""}`}>
          <div className="toast-title">
            <span>{t.glyph}</span>
            <span>achievement unlocked</span>
          </div>
          <div className="toast-name">{t.title}</div>
          <div className="toast-desc">{t.desc}</div>
        </div>
      ))}
    </div>
  );
}

// ── Onboarding overlay ─────────────────────────────────────────────────────
// Three-beat first-run: streamed `npm install -g tokenpet` → name your egg →
// crack-and-hatch animation → drop into the live terminal.
function OnboardingOverlay({ defaultName, onFinish, onSkip }) {
  const [phase, setPhase] = _useState("install"); // install → name → hatch
  const [installLines, setInstallLines] = _useState([]);
  const [name, setName] = _useState(defaultName || "");
  const [cracking, setCracking] = _useState(false);
  const inputRef = _useRef(null);

  // Stream the fake install lines.
  _useEffect(() => {
    if (phase !== "install") return;
    const lines = [
      { t: "$ npm install -g glorp", k: "il-cmd" },
      { t: "→ resolving dependencies…", k: "il-info" },
      { t: "  added @glorp/core@1.0.0", k: "il-ok" },
      { t: "  added @glorp/render-ascii@0.4.2", k: "il-ok" },
      { t: "  linked claude-code adapter", k: "il-ok" },
      { t: "  linked codex-cli adapter", k: "il-ok" },
      { t: "→ scanning for token usage history…", k: "il-info" },
      { t: "  found 7 days of usage · ~1.9M tokens", k: "il-ok" },
      { t: "  warming up pet incubator…", k: "il-info" },
      { t: "✓ glorp installed.", k: "il-ok" },
      { t: "", k: "il-info" },
      { t: "$ glorp init", k: "il-cmd" },
      { t: "→ a new egg appears in your terminal:", k: "il-info" },
      { t: "", k: "il-info" },
      { t: "        .---.        ", k: "il-egg" },
      { t: "       /     \\       ", k: "il-egg" },
      { t: "      | ◆ ◆ |      ", k: "il-egg" },
      { t: "       \\_._./        ", k: "il-egg" },
      { t: "", k: "il-info" },
      { t: "→ name your pet to hatch it.", k: "il-warn" },
    ];
    let idx = 0;
    const interval = setInterval(() => {
      idx++;
      setInstallLines(lines.slice(0, idx));
      if (idx >= lines.length) {
        clearInterval(interval);
        setTimeout(() => setPhase("name"), 350);
      }
    }, 110);
    return () => clearInterval(interval);
  }, [phase]);

  _useEffect(() => {
    if (phase === "name" && inputRef.current) inputRef.current.focus();
  }, [phase]);

  const submit = () => {
    const n = (name || defaultName || "egg").trim().toLowerCase().slice(0, 16);
    setCracking(true);
    bellFlash();
    setTimeout(() => onFinish(n), 950);
  };

  return (
    <div className="onboarding">
      <div className="onboarding-card">
        {phase === "install" && (
          <div className="install-stream">
            {installLines.map((l, i) => (
              <div key={i} className={l.k}>{l.t || "\u00a0"}</div>
            ))}
          </div>
        )}
        {phase === "name" && !cracking && (
          <div>
            <div className="install-stream" style={{minHeight:"auto"}}>
              <div className="il-info">→ name your pet to hatch it.</div>
            </div>
            <div className="onboarding-egg">
              {`        .---.\n       /     \\\n      | ◆ ◆ |\n       \\_._./`}
            </div>
            <input
              ref={inputRef}
              className="onboarding-name-input"
              placeholder={defaultName}
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") submit(); }}
              maxLength={16}
            />
            <div className="onboarding-name-hint">
              press <code>↵</code> to hatch · or pick the seeded name <code>{defaultName}</code>
            </div>
            <div className="onboarding-actions">
              <button className="ghost" onClick={onSkip}>skip</button>
              <button onClick={submit}>hatch ↵</button>
            </div>
          </div>
        )}
        {phase === "name" && cracking && (
          <div>
            <div className="install-stream" style={{minHeight:"auto"}}>
              <div className="il-warn">→ hatching {name || defaultName}…</div>
            </div>
            <div className="onboarding-egg cracking">
              {`        .---.\n       /  *  \\\n      | ✦ ✦ |\n       \\_*_./`}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Death overlay ──────────────────────────────────────────────────────────
function DeathOverlay({ pet, age, cause, onRevive, onLetGo }) {
  return (
    <div className="death-overlay">
      <div className="rip-card">
        <div className="rip-headline">⚠ critical · pet has perished</div>
        <pre className="rip-stone">{`   _______
  /       \\
 |   R I P  |
 |          |
 |  ${(pet.name || "—").padEnd(7,' ').slice(0,7)} |
 |  ${(age   || "—").padEnd(7,' ').slice(0,7)} |
 |__________|`}</pre>
        <div style={{color:"var(--dim)", fontSize:"11px", margin:"6px 0 14px"}}>
          cause: <span style={{color:"var(--bad)"}}>{cause}</span>
        </div>
        <div style={{display:"flex", gap:"8px", justifyContent:"center"}}>
          <button className="rip-revive" onClick={onRevive}>♺ revive (-50% stats)</button>
          <button className="rip-revive" style={{borderColor:"var(--faint)", color:"var(--dim)"}} onClick={onLetGo}>let go · new egg</button>
        </div>
      </div>
    </div>
  );
}

// ── Treat menu ─────────────────────────────────────────────────────────────
function TreatMenu({ onPick }) {
  return (
    <div className="treat-menu">
      {TREATS.map((tr) => (
        <button key={tr.id} onClick={() => onPick(tr.id)} title={tr.desc}>
          <span className="treat-glyph">{tr.glyph}</span>
          <span>{tr.label}</span>
        </button>
      ))}
    </div>
  );
}

// Expose to window for app.jsx (Babel scripts get isolated scope per-tag).
Object.assign(window, {
  ACHIEVEMENTS, TREATS, computeUnlocks, bellFlash,
  Sparkline, HintStrip, HelpPalette, ToastStack,
  OnboardingOverlay, DeathOverlay, TreatMenu,
});
