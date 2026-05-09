// app.jsx — the tokenpet CLI prototype.
// State management, simulated activity, command bar, log feed.

const { useState, useEffect, useMemo, useRef, useCallback } = React;

// Components/constants from extras.jsx (loaded just before this script).
const {
  Sparkline, HintStrip, HelpPalette, ToastStack, OnboardingOverlay,
  DeathOverlay, TreatMenu, ACHIEVEMENTS, TREATS, bellFlash, computeUnlocks,
} = window;

// ── Defaults a fresh user lands on ─────────────────────────────────────────
// Pup stage with healthy stats — gives the user a real-feeling middle state
// to interact with rather than starting them at an empty egg.
const DEFAULT_STATE = {
  seed: "mochi-7f3a",
  customName: null, // null → use seed-generated name
  bornAt: Date.now() - 1000 * 60 * 60 * 24 * 12 - 1000 * 60 * 60 * 4, // 12d 4h
  lifetimeTokens: 1_847_320,
  todayTokens: 412_847,
  tokensYesterday: 335_900,
  prompts: 87,
  sessions: 9,
  commits: 14,
  prsMerged: 3,
  linesAdded: 1247,
  linesRemoved: 382,
  happiness: 78,
  fed: 62,
  energy: 88,
  // 7-day token history (oldest → today). Drives the today-box sparkline.
  tokenHistory: [185_300, 220_400, 410_800, 96_500, 280_200, 335_900, 412_847],
  // ids of achievements already unlocked when the user lands.
  achievements: ["first_feed", "first_ship", "tok_1m"],
  // Misc flags for stickier achievements (night-owl feed, stage hits, etc).
  flags: { metSpecies: ["fuzz"] },
  // Past pets. Each: { name, species, age, cause, glyph }
  graveyard: [],
  // Used by deriveMood + isDead — refreshed on any user-driven action.
  lastInteractionAt: Date.now() - 1000 * 60 * 30,
  // Hard kill flag — the death overlay reads this. cmdRevive clears it.
  dead: false,
};

// ── A curated litter of seed strings — different species mix on display.
// Hand-picked to ensure visual variety; the generator is deterministic so
// these always grow the same pets.
const LITTER_SEEDS = [
  "mochi-7f3a",
  "kib-spectre",
  "void.0xff",
  "puffball-9",
  "ori-shard",
  "tam-glitch",
  "biz-cube",
];

// ── Tweak defaults — host rewrites this JSON block on persist ──────────────
const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "warm",
  "stageOverride": "auto",
  "speciesOverride": "auto",
  "seed": "mochi-7f3a",
  "speed": 1500,
  "showCursor": true,
  "showLitter": true,
  "showTreats": true
}/*EDITMODE-END*/;

const THEMES = {
  warm: {
    bg: "oklch(0.18 0.005 60)",
    surface: "oklch(0.22 0.006 60)",
    fg: "oklch(0.94 0.01 80)",
    dim: "oklch(0.66 0.012 70)",
    faint: "oklch(0.42 0.008 60)",
    accent: "oklch(0.78 0.14 70)",   // amber
    good: "oklch(0.74 0.10 145)",    // moss green
    bad: "oklch(0.68 0.16 25)",      // coral
    crt: false,
  },
  green: {
    bg: "#0a0f08",
    surface: "#0e1610",
    fg: "oklch(0.86 0.18 145)",
    dim: "oklch(0.62 0.14 145)",
    faint: "oklch(0.38 0.08 145)",
    accent: "oklch(0.92 0.18 130)",
    good: "oklch(0.86 0.18 145)",
    bad: "oklch(0.78 0.20 30)",
    crt: true,
  },
  amber: {
    bg: "#0f0a05",
    surface: "#170f06",
    fg: "oklch(0.84 0.16 75)",
    dim: "oklch(0.62 0.12 75)",
    faint: "oklch(0.38 0.08 75)",
    accent: "oklch(0.92 0.16 90)",
    good: "oklch(0.78 0.13 130)",
    bad: "oklch(0.72 0.18 35)",
    crt: true,
  },
  ice: {
    bg: "oklch(0.20 0.012 240)",
    surface: "oklch(0.24 0.014 240)",
    fg: "oklch(0.94 0.02 220)",
    dim: "oklch(0.66 0.04 220)",
    faint: "oklch(0.42 0.02 230)",
    accent: "oklch(0.78 0.13 230)",
    good: "oklch(0.78 0.10 180)",
    bad: "oklch(0.72 0.14 25)",
    crt: false,
  },
};

// ── Number formatting ──────────────────────────────────────────────────────
function fmt(n) {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(n >= 10_000 ? 0 : 1) + "k";
  return String(n);
}
function fmtComma(n) {
  return n.toLocaleString("en-US");
}
function ageString(ts) {
  const ms = Date.now() - ts;
  const d = Math.floor(ms / (1000 * 60 * 60 * 24));
  const h = Math.floor((ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
  return `${d}d ${h}h old`;
}
function timeStr(d = new Date()) {
  return d.toTimeString().slice(0, 5);
}

// ── Mood derivation ────────────────────────────────────────────────────────
// Mood is mostly derived from current stats + recent action. The transient
// `flash` mood (set via setFlash) overrides for a short window after feeding/
// shipping so the pet visibly reacts.
function deriveMood({ happiness, fed, energy, lastInteractionAt }) {
  // Neglect: untouched > ~36h with low fed AND low happy reads as wilted.
  // This is the "very sad and tired" state — not death, just visibly hurt.
  const stale = lastInteractionAt
    ? (Date.now() - lastInteractionAt) > 36 * 3600 * 1000
    : false;
  if (stale && fed < 35 && happiness < 35) return "wilted";
  if (fed < 12 && happiness < 20) return "wilted";
  if (energy < 18) return "sleepy";
  if (fed < 25) return "hungry";
  if (happiness < 35) return "sad";
  if (happiness > 85 && fed > 60) return "happy";
  return "content";
}

// ── Bar widget ─────────────────────────────────────────────────────────────
// Pure unicode block bars — match terminal aesthetic. 20 cells total.
function Bar({ value, color }) {
  const cells = 20;
  const filled = Math.round((Math.max(0, Math.min(100, value)) / 100) * cells);
  const empty = cells - filled;
  return (
    <span className="bar" style={{ color }}>
      <span style={{ color }}>{"█".repeat(filled)}</span>
      <span className="bar-empty">{"░".repeat(empty)}</span>
    </span>
  );
}

// ── StatRow ────────────────────────────────────────────────────────────────
function StatRow({ icon, label, value, max = 100, color, suffix }) {
  return (
    <div className="stat-row">
      <span className="stat-icon">{icon}</span>
      <span className="stat-label">{label}</span>
      <Bar value={value} color={color} />
      <span className="stat-value">
        {Math.round(value)}%{suffix && <span className="stat-suffix"> {suffix}</span>}
      </span>
    </div>
  );
}

// ── App ────────────────────────────────────────────────────────────────────
function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const theme = THEMES[t.theme] || THEMES.warm;

  const [state, setState] = useState(DEFAULT_STATE);
  const [flash, setFlash] = useState(null); // {mood, until}
  const [bubble, setBubble] = useState(null); // {text, until}
  const [log, setLog] = useState(() => initialLog());
  const [cmdInput, setCmdInput] = useState("");
  const [history, setHistory] = useState([]);
  const [histIdx, setHistIdx] = useState(-1);
  const logRef = useRef(null);
  // Cross-cutting overlays + toast queue.
  const [showHelp, setShowHelp] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [toasts, setToasts] = useState([]);

  // ── Apply theme as CSS vars at document level so children inherit cleanly.
  useEffect(() => {
    const r = document.documentElement;
    Object.entries(theme).forEach(([k, v]) => {
      if (typeof v === "string") r.style.setProperty(`--${k}`, v);
    });
    document.body.classList.toggle("crt", !!theme.crt);
  }, [theme]);

  // Sync seed from tweak — host edits in tweaks panel propagate to state.
  useEffect(() => {
    if (t.seed && t.seed !== state.seed) {
      setState((s) => ({ ...s, seed: t.seed, customName: null }));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [t.seed]);

  // Generated pet — deterministic from seed. Then optional species override
  // from tweaks lets you swap silhouette without losing eyes/pattern/color.
  const generated = useMemo(() => generatePet(state.seed), [state.seed]);
  const pet = useMemo(() => {
    const p = { ...generated };
    if (t.speciesOverride && t.speciesOverride !== "auto") p.species = t.speciesOverride;
    if (state.customName) p.name = state.customName;
    return p;
  }, [generated, t.speciesOverride, state.customName]);

  // The display name — custom override > seeded name.
  const displayName = state.customName || pet.name;

  // ── Daily pace: auto-calibrate evolution arc to the user's actual usage.
  // A 500M/day user shouldn't outgrow the arc in 4 days; a 50k/day user
  // shouldn't be stuck as a hatchling forever. Pace floors at the default.
  const dailyPace = Math.max(
    100_000,
    state.tokensYesterday || 0,
    state.todayTokens || 0
  );

  // ── Stage: either auto-derived from lifetimeTokens, or pinned via tweak.
  const autoStage = stageFromTokens(state.lifetimeTokens, dailyPace);
  const stage = t.stageOverride && t.stageOverride !== "auto"
    ? t.stageOverride
    : autoStage;

  // ── Cinematic evolution overlay: detect stage transition, show splash.
  const [evo, setEvo] = useState(null);
  const prevStageRef = useRef(stage);
  useEffect(() => {
    const prev = prevStageRef.current;
    if (prev && prev !== stage && STAGES.indexOf(stage) > STAGES.indexOf(prev)) {
      setEvo({ from: prev, to: stage, until: Date.now() + 4200 });
      bellFlash();
      // Pin a stage flag so the achievement check can see it later.
      setState((s) => {
        const flags = { ...s.flags };
        if (stage === "s3") flags.stagePup = true;
        if (stage === "s4" || stage === "s5") flags.stageAdult = true;
        if (stage === "s6") flags.stageSage = true;
        return { ...s, flags };
      });
    }
    prevStageRef.current = stage;
  }, [stage]);
  useEffect(() => {
    if (!evo) return;
    const id = setTimeout(() => setEvo(null), Math.max(0, evo.until - Date.now()));
    return () => clearTimeout(id);
  }, [evo]);

  const baseMood = deriveMood(state);
  const isDead = !!state.dead;
  const mood = isDead ? "dead" : (flash && flash.until > Date.now() ? flash.mood : baseMood);
  const bubbleText = bubble && bubble.until > Date.now() ? bubble.text : null;

  // Clear flash/bubble when their timer expires.
  useEffect(() => {
    if (!flash) return;
    const ms = flash.until - Date.now();
    if (ms <= 0) { setFlash(null); return; }
    const id = setTimeout(() => setFlash(null), ms);
    return () => clearTimeout(id);
  }, [flash]);
  useEffect(() => {
    if (!bubble) return;
    const ms = bubble.until - Date.now();
    if (ms <= 0) { setBubble(null); return; }
    const id = setTimeout(() => setBubble(null), ms);
    return () => clearTimeout(id);
  }, [bubble]);

  // ── Track distinct species the user has met for the "collector" achievement.
  useEffect(() => {
    if (!generated?.species) return;
    setState((s) => {
      const met = new Set(s.flags?.metSpecies || []);
      if (met.has(generated.species)) return s;
      met.add(generated.species);
      return { ...s, flags: { ...s.flags, metSpecies: Array.from(met) } };
    });
  }, [generated?.species]);

  // ── Detect newly-unlocked achievements; toast each one with a stagger.
  // The `newly.length === 0` guard prevents this effect from looping when
  // achievements update (it only runs setState if there's actually new work).
  useEffect(() => {
    const prevSet = new Set(state.achievements || []);
    const { set, newly } = computeUnlocks(state, prevSet);
    if (newly.length === 0) return;
    setState((s) => ({ ...s, achievements: Array.from(set) }));
    newly.forEach((a, i) => {
      const id = `${a.id}-${Date.now()}`;
      setTimeout(() => {
        setToasts((ts) => [...ts, { ...a, id }]);
        bellFlash();
        pushLog(`✦ achievement: ${a.title} — ${a.desc}`, "achv");
        setTimeout(() => setToasts((ts) => ts.map((t) => t.id === id ? { ...t, exit: true } : t)), 4400);
        setTimeout(() => setToasts((ts) => ts.filter((t) => t.id !== id)), 4900);
      }, i * 350);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.lifetimeTokens, state.prsMerged, state.fed, state.happiness, state.flags]);

  // ── Global '?' key opens the help palette (when input isn't focused).
  useEffect(() => {
    const onKey = (e) => {
      if (e.key === "?" && !(["INPUT", "TEXTAREA"].includes(e.target?.tagName))) {
        e.preventDefault();
        setShowHelp(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // ── Sim tick: hunger + energy drift, plus occasional ambient log lines.
  useEffect(() => {
    const id = setInterval(() => {
      setState((s) => {
        const fed = Math.max(0, s.fed - 1.2);
        const energy = Math.max(0, s.energy - 0.4);
        // Happiness drifts toward equilibrium based on fed/energy.
        const target = (fed + energy) / 2;
        const happiness = s.happiness + (target - s.happiness) * 0.05;
        return { ...s, fed, energy, happiness };
      });
    }, 4000);
    return () => clearInterval(id);
  }, []);

  // Ambient chatter to make the feed feel alive.
  useEffect(() => {
    const id = setInterval(() => {
      const lines = ambientLines(stage, baseMood, displayName);
      const pick = lines[Math.floor(Math.random() * lines.length)];
      if (pick) pushLog(pick.text, pick.kind);
    }, 9000);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stage, baseMood, displayName]);

  const pushLog = useCallback((text, kind = "info") => {
    setLog((l) => {
      const next = [...l, { time: timeStr(), text, kind }];
      return next.length > 200 ? next.slice(-200) : next;
    });
  }, []);

  // Auto-scroll log to bottom on update.
  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [log]);

  // ── Commands ─────────────────────────────────────────────────────────────
  const cmdFeed = (amount = 47_000) => {
    bellFlash();
    setState((s) => {
      const lifetime = s.lifetimeTokens + amount;
      const today = s.todayTokens + amount;
      const fed = Math.min(100, s.fed + 18);
      const happiness = Math.min(100, s.happiness + 6);
      const prompts = s.prompts + Math.max(1, Math.floor(amount / 5000));
      const pace = Math.max(100_000, s.tokensYesterday || 0, s.todayTokens || 0);
      const wasStage = stageFromTokens(s.lifetimeTokens, pace);
      const nowStage = stageFromTokens(lifetime, pace);
      if (wasStage !== nowStage) {
        setTimeout(() => pushLog(`★ ${s.name} evolved: ${wasStage} → ${nowStage}!`, "level"), 250);
      }
      // Append today's running total to the 7-day rolling history.
      const tokenHistory = [...((s.tokenHistory || []).slice(-6)), today];
      // Mark night-owl flag if it's between midnight and 5am locally.
      const hr = new Date().getHours();
      const flags = { ...s.flags };
      if (hr >= 0 && hr < 5) flags.nightOwl = true;
      return { ...s, lifetimeTokens: lifetime, todayTokens: today, fed, happiness, prompts, tokenHistory, flags, lastInteractionAt: Date.now() };
    });
    setFlash({ mood: "happy", until: Date.now() + 2200 });
    setBubble({ text: pickBubble(["yum!", "more!", "tasty tokens", "*chomp*", "nom"]), until: Date.now() + 2000 });
    pushLog(`${displayName} devoured ${fmt(amount)} tokens (yum)`, "feed");
  };

  const cmdShip = () => {
    bellFlash();
    setState((s) => ({
      ...s,
      lastInteractionAt: Date.now(),
      prsMerged: s.prsMerged + 1,
      commits: s.commits + Math.floor(2 + Math.random() * 6),
      linesAdded: s.linesAdded + Math.floor(80 + Math.random() * 400),
      linesRemoved: s.linesRemoved + Math.floor(20 + Math.random() * 200),
      happiness: Math.min(100, s.happiness + 22),
      energy: Math.min(100, s.energy + 8),
    }));
    setFlash({ mood: "ecstatic", until: Date.now() + 2800 });
    setBubble({ text: pickBubble(["shipped!! ✦", "we did it!", "yesss", "go go go", "★ ★ ★"]), until: Date.now() + 2500 });
    pushLog(`PR merged → +50 happy, ${displayName} is ecstatic`, "ship");
  };

  const cmdPet = () => {
    setState((s) => ({ ...s, happiness: Math.min(100, s.happiness + 4) }));
    setFlash({ mood: "happy", until: Date.now() + 1400 });
    setBubble({ text: pickBubble(["♡", "purr~", "hi :)", "*nuzzle*"]), until: Date.now() + 1400 });
    pushLog(`you pet ${displayName}. it purrs.`, "info");
  };

  const cmdSleep = () => {
    setState((s) => ({
      ...s,
      energy: Math.min(100, s.energy + 30),
      fed: Math.max(0, s.fed - 8),
    }));
    setFlash({ mood: "sleepy", until: Date.now() + 2400 });
    setBubble({ text: pickBubble(["zzz...", "dreaming", "z z z"]), until: Date.now() + 2400 });
    pushLog(`${displayName} took a nap. +30 energy.`, "info");
  };

  const cmdStatus = () => {
    pushLog(
      `${displayName} · ${stageLabel(pet, stage)} · ${ageString(state.bornAt)} · happy ${Math.round(state.happiness)}% fed ${Math.round(state.fed)}% energy ${Math.round(state.energy)}%`,
      "info"
    );
  };

  const cmdTreat = (id) => {
    const tr = TREATS.find((x) => x.id === id);
    if (!tr) {
      pushLog(`treats: ${TREATS.map((x) => x.id).join(", ")} — try "treat espresso"`, "help");
      return;
    }
    bellFlash();
    setState((s) => ({
      ...s,
      energy:    Math.max(0, Math.min(100, s.energy + tr.fx.energy)),
      fed:       Math.max(0, Math.min(100, s.fed + tr.fx.fed)),
      happiness: Math.max(0, Math.min(100, s.happiness + tr.fx.happiness)),
      lifetimeTokens: s.lifetimeTokens + tr.fx.tokens,
      todayTokens: s.todayTokens + tr.fx.tokens,
      lastInteractionAt: Date.now(),
    }));
    setFlash({ mood: tr.fx.happiness >= 12 ? "ecstatic" : "happy", until: Date.now() + 1800 });
    setBubble({ text: pickBubble(["yum!", "thanks!", "*nom*", "more?"]), until: Date.now() + 1800 });
    pushLog(`${displayName} got a ${tr.label} ${tr.glyph} — ${tr.desc}`, "treat");
  };

  const cmdRevive = () => {
    bellFlash();
    setState((s) => ({
      ...s,
      dead: false,
      fed: 50,
      energy: 50,
      happiness: 60,
      todayTokens: 0,
      lastInteractionAt: Date.now(),
      flags: { ...s.flags, revived: true },
    }));
    pushLog(`★ ${displayName} revived. spend tokens carefully.`, "level");
  };

  const cmdAchievements = () => {
    const set = new Set(state.achievements || []);
    pushLog(`achievements: ${set.size}/${ACHIEVEMENTS.length} unlocked`, "info");
    ACHIEVEMENTS.forEach((a) => {
      const u = set.has(a.id);
      pushLog(`  ${u ? "✓" : "·"} ${a.glyph} ${a.title} — ${a.desc}`, u ? "achv" : "help");
    });
  };

  const cmdGraveyard = () => {
    if (!state.graveyard.length) {
      pushLog(`graveyard is empty. (your pet still lives.)`, "info");
      return;
    }
    pushLog(`graveyard:`, "info");
    state.graveyard.forEach((g) => {
      pushLog(`  ${g.glyph || "✦"} ${g.name} · ${g.species} · ${g.cause} · ${g.age}`, "info");
    });
  };

  const cmdHelp = () => { setShowHelp(true); };

  const runCommand = (raw) => {
    const line = raw.trim();
    if (!line) return;
    pushLog(`$ ${line}`, "cmd");
    setHistory((h) => [...h, line]);
    setHistIdx(-1);

    const [cmd, ...args] = line.split(/\s+/);
    const c = cmd.toLowerCase().replace(/^(glorp|tokenpet)$/, "");
    // Allow `glorp feed` or just `feed`.
    const sub = c === "" ? args.shift()?.toLowerCase() : c;

    switch (sub) {
      case undefined:
      case "status": cmdStatus(); break;
      case "feed": {
        const n = args[0] ? parseTokenAmount(args[0]) : 47_000;
        cmdFeed(n);
        break;
      }
      case "ship": cmdShip(); break;
      case "pet": cmdPet(); break;
      case "sleep": case "nap": cmdSleep(); break;
      case "help": case "?": cmdHelp(); break;
      case "treat": cmdTreat(args[0]?.toLowerCase()); break;
      case "revive": cmdRevive(); break;
      case "achievements": case "achv": cmdAchievements(); break;
      case "graveyard": case "gy": cmdGraveyard(); break;
      case "clear": setLog([]); break;
      case "rename": {
        const newName = args.join(" ").trim() || pet.name;
        setState((s) => ({ ...s, customName: newName }));
        pushLog(`renamed → ${newName}`, "info");
        break;
      }
      case "reroll": case "seed": {
        const newSeed = args.join(" ").trim() || Math.random().toString(36).slice(2, 10);
        setState((s) => ({ ...s, seed: newSeed, customName: null }));
        setTweak("seed", newSeed);
        pushLog(`seed → ${newSeed} · new pet incoming...`, "level");
        break;
      }
      default:
        pushLog(`unknown command: ${sub}. try 'help'.`, "err");
    }
  };

  const onCmdKey = (e) => {
    if (e.key === "Enter") {
      runCommand(cmdInput);
      setCmdInput("");
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const next = histIdx < 0 ? history.length - 1 : Math.max(0, histIdx - 1);
      if (history[next] != null) { setHistIdx(next); setCmdInput(history[next]); }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      const next = histIdx + 1;
      if (next >= history.length) { setHistIdx(-1); setCmdInput(""); }
      else { setHistIdx(next); setCmdInput(history[next]); }
    } else if (e.key === "l" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      setLog([]);
    }
  };

  // ── Derived display values ───────────────────────────────────────────────
  const yPct = state.tokensYesterday > 0
    ? Math.round(((state.todayTokens - state.tokensYesterday) / state.tokensYesterday) * 100)
    : 0;
  const { next, threshold } = nextStageInfo(state.lifetimeTokens, dailyPace);
  const stageInfo = next
    ? `${fmt(state.lifetimeTokens)} / ${fmt(threshold)} to ${next}`
    : `${fmt(state.lifetimeTokens)} · sage form ✦`;

  return (
    <div className="screen">
      {evo ? <EvolutionOverlay pet={pet} from={evo.from} to={evo.to} /> : null}
      <ToastStack toasts={toasts} />
      {showHelp && <HelpPalette onClose={() => setShowHelp(false)} />}
      {showOnboarding && (
        <OnboardingOverlay
          defaultName={pet.name}
          onFinish={(n) => {
            setState((s) => ({ ...s, customName: n, lastInteractionAt: Date.now() }));
            setShowOnboarding(false);
            pushLog(`★ welcome ${n}!`, "level");
          }}
          onSkip={() => setShowOnboarding(false)}
        />
      )}
      {isDead && (
        <DeathOverlay
          pet={{ name: displayName }}
          age={ageString(state.bornAt)}
          cause="starvation"
          onRevive={() => cmdRevive()}
          onLetGo={() => {
            setState((s) => ({
              ...s,
              dead: false,
              graveyard: [...(s.graveyard||[]), { name: displayName, species: pet.species, age: ageString(state.bornAt), cause: "starvation", glyph: "✦" }],
              seed: Math.random().toString(36).slice(2,10),
              customName: null,
              fed: 70, energy: 70, happiness: 70,
              lifetimeTokens: 0, todayTokens: 0, prsMerged: 0, commits: 0, prompts: 0,
              bornAt: Date.now(),
              lastInteractionAt: Date.now(),
            }));
            pushLog(`☆ a new egg appears in your terminal.`, "info");
          }}
        />
      )}
      <div className="terminal">
        {/* ── window chrome ─────────────────────────────────────── */}
        <div className="chrome">
          <div className="chrome-buttons">
            <span className="cb red" />
            <span className="cb yel" />
            <span className="cb grn" />
          </div>
          <div className="chrome-title">glorp — {displayName.toLowerCase()}@claude:~ — 80×30</div>
          <div className="chrome-spacer" />
        </div>

        {/* ── body ──────────────────────────────────────────────── */}
        <div className="body">
          <div className="cmdline-prompt">
            <span className="prompt-user">{displayName.toLowerCase()}@claude</span>
            <span className="prompt-sep">:</span>
            <span className="prompt-path">~</span>
            <span className="prompt-sep">$</span>
            <span className="prompt-typed"> glorp</span>
          </div>

          {t.showLitter && (
            <div className="litter">
              <div className="section-header">─ litter ─ click to adopt ──────────────────────────</div>
              <div className="litter-row">
                {LITTER_SEEDS.map((s) => {
                  const p = generatePet(s);
                  const active = s === state.seed;
                  return (
                    <button
                      key={s}
                      type="button"
                      className={`mini-card ${active ? "active" : ""}`}
                      onClick={() => {
                        setState((st) => ({ ...st, seed: s, customName: null }));
                        setTweak("seed", s);
                        pushLog(`adopted ${p.name} · ${SPECIES[p.species].label}`, "level");
                      }}
                      title={`${p.name} · ${SPECIES[p.species].label} · seed=${p.seed}`}
                    >
                      <MiniPet pet={p} stage="adult" />
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          <div className="columns">
            {/* ── LEFT: pet + vitals ─────────────────────────── */}
            <div className="col">
              <div className="pet-stage">
                <Pet pet={pet} stage={stage} mood={mood} bubble={bubbleText} speed={t.speed} />
                <div className="pet-meta">
                  <span className="pet-name">{displayName}</span>
                  <span className="pet-sep">·</span>
                  <span className="pet-stage-tag">{stageLabel(pet, stage)}</span>
                  <span className="pet-sep">·</span>
                  <span className="pet-age">{ageString(state.bornAt)}</span>
                  <span className="pet-sep">·</span>
                  <span className={`pet-mood mood-${mood}`}>
                    {moodLabel(mood)}
                  </span>
                </div>
              </div>

              <div className="section">
                <div className="section-header">─ vitals ─────────────────────────────</div>
                <StatRow icon="♥" label="happy " value={state.happiness} color="var(--accent)" />
                <StatRow icon="◉" label="fed   " value={state.fed} color="var(--good)" />
                <StatRow icon="⚡" label="energy" value={state.energy} color="var(--accent)" />
                <div className="stat-row stat-xp">
                  <span className="stat-icon">✦</span>
                  <span className="stat-label">xp    </span>
                  <Bar
                    value={xpPercent(state.lifetimeTokens, dailyPace)}
                    color="var(--accent)"
                  />
                  <span className="stat-value xp-value">{stageInfo}</span>
                </div>
              </div>
            </div>

            {/* ── RIGHT: today + feed + actions ──────────────── */}
            <div className="col">
              <div className="section">
            <div className="section-header">─ today ──────────────────────────────────────────</div>
            <div className="today-grid">
              <div className="today-cell">
                <span className="today-label">tokens</span>
                <span className="today-value">{fmtComma(state.todayTokens)}</span>
                <span className={`today-delta ${yPct >= 0 ? "up" : "down"}`}>
                  <Sparkline values={state.tokenHistory} />
                  {" "}{yPct >= 0 ? "↑" : "↓"} {Math.abs(yPct)}%
                </span>
              </div>
              <div className="today-cell">
                <span className="today-label">prompts</span>
                <span className="today-value">{state.prompts}</span>
                <span className="today-delta">across {state.sessions} sessions</span>
              </div>
              <div className="today-cell">
                <span className="today-label">commits</span>
                <span className="today-value">{state.commits}</span>
                <span className="today-delta">{state.prsMerged} PRs merged</span>
              </div>
              <div className="today-cell">
                <span className="today-label">diff</span>
                <span className="today-value">
                  <span className="good">+{fmt(state.linesAdded)}</span>
                  {" "}
                  <span className="bad">-{fmt(state.linesRemoved)}</span>
                </span>
                <span className="today-delta">lines shipped</span>
              </div>
            </div>
          </div>

          {/* Live log */}
          <div className="section log-section">
            <div className="section-header">─ feed ───────────────────────────────────────────</div>
            <div className="log" ref={logRef}>
              {log.map((entry, i) => (
                <div key={i} className={`log-entry log-${entry.kind}`}>
                  <span className="log-time">{entry.time}</span>
                  <span className="log-text">{entry.text}</span>
                </div>
              ))}
              <div className="log-entry log-prompt">
                <span className="log-time">{timeStr()}</span>
                <span className="log-text">
                  <span className="prompt-sep">$</span>{" "}
                  <input
                    className="cmd-input"
                    value={cmdInput}
                    onChange={(e) => setCmdInput(e.target.value)}
                    onKeyDown={onCmdKey}
                    spellCheck={false}
                    autoFocus
                    placeholder="type 'help' — or click a button below"
                  />
                  {t.showCursor && <span className="caret">█</span>}
                </span>
              </div>
            </div>
          </div>

          {/* Quick actions */}
          <div className="actions">
            <button onClick={() => runCommand("feed")}>feed</button>
            <button onClick={() => runCommand("ship")}>ship</button>
            <button onClick={() => runCommand("pet")}>pet</button>
            <button onClick={() => runCommand("sleep")}>sleep</button>
            <button onClick={() => runCommand("status")} className="ghost">status</button>
            <button onClick={() => setShowHelp(true)} className="ghost">help</button>
          </div>
          {t.showTreats && (
            <div className="section">
              <div className="section-header">─ treats ─ click to give ────────────────────</div>
              <TreatMenu onPick={(id) => runCommand(`treat ${id}`)} />
            </div>
          )}
            </div>
          </div>
          <HintStrip onHelp={() => setShowHelp(true)} />
        </div>
      </div>

      {/* ── Tweaks ─────────────────────────────────────────────── */}
      <TweaksPanel title="Tweaks">
        <TweakSection label="Theme">
          <TweakRadio
            label="palette"
            value={t.theme}
            options={[
              { value: "warm",  label: "warm" },
              { value: "green", label: "crt" },
              { value: "amber", label: "amber" },
              { value: "ice",   label: "ice" },
            ]}
            onChange={(v) => setTweak("theme", v)}
          />
        </TweakSection>

        <TweakSection label="Pet">
          <TweakText
            label="seed"
            value={t.seed}
            onChange={(v) => setTweak("seed", v)}
          />
          <TweakButton label="reroll seed" secondary onClick={() => {
            const s = Math.random().toString(36).slice(2, 10);
            setTweak("seed", s);
            setState((st) => ({ ...st, seed: s, customName: null }));
          }} />
          <TweakSelect
            label="species"
            value={t.speciesOverride}
            options={[
              { value: "auto", label: "auto (from seed)" },
              ...SPECIES_LIST.map((s) => ({ value: s, label: s })),
            ]}
            onChange={(v) => setTweak("speciesOverride", v)}
          />
          <TweakSelect
            label="stage"
            value={t.stageOverride}
            options={[
              { value: "auto",      label: "auto (from tokens)" },
              { value: "s0",        label: "s0 · newborn" },
              { value: "s1",        label: "s1" },
              { value: "s2",        label: "s2" },
              { value: "s3",        label: "s3 · pup" },
              { value: "s4",        label: "s4 · adult" },
              { value: "s5",        label: "s5 · elder" },
              { value: "s6",        label: "s6 · mythic" },
              { value: "__hr",      label: "── legacy ──", disabled: true },
              { value: "egg",       label: "egg" },
              { value: "hatchling", label: "hatchling" },
              { value: "pup",       label: "pup" },
              { value: "adult",     label: "adult" },
              { value: "sage",      label: "sage" },
            ]}
            onChange={(v) => setTweak("stageOverride", v)}
          />
          <TweakSlider
            label="breath speed"
            value={t.speed}
            min={400} max={3000} step={100} unit="ms"
            onChange={(v) => setTweak("speed", v)}
          />
          <TweakToggle
            label="blinking cursor"
            value={t.showCursor}
            onChange={(v) => setTweak("showCursor", v)}
          />
          <TweakToggle
            label="show litter"
            value={t.showLitter}
            onChange={(v) => setTweak("showLitter", v)}
          />
        </TweakSection>

        <TweakSection label="Simulate">
          <TweakButton label="burn 500k tokens" onClick={() => cmdFeed(500_000)} />
          <TweakButton label="burn 5M tokens"   onClick={() => cmdFeed(5_000_000)} />
          <TweakButton label="ship 3 PRs" secondary onClick={() => { cmdShip(); setTimeout(cmdShip, 200); setTimeout(cmdShip, 400); }} />
          <TweakButton label="kill (starve)" secondary onClick={() => setState((s) => ({ ...s, fed: 0, happiness: 0, energy: 0, dead: true }))} />
          <TweakButton label="replay first-run" secondary onClick={() => setShowOnboarding(true)} />
          <TweakButton label="reset" secondary onClick={() => { setState(DEFAULT_STATE); setLog(initialLog()); }} />
          <TweakToggle label="show treats menu" value={t.showTreats} onChange={(v) => setTweak("showTreats", v)} />
        </TweakSection>

        <TweakSection label="Achievements">
          <div style={{fontSize:"11px", color:"var(--dim)", marginBottom:"6px"}}>
            {(state.achievements||[]).length}/{ACHIEVEMENTS.length} unlocked
          </div>
          <div className="achv-list">
            {ACHIEVEMENTS.map((a) => {
              const unlocked = (state.achievements||[]).includes(a.id);
              return (
                <div key={a.id} className={`achv-row ${unlocked ? "" : "locked"}`}>
                  <span className="ag">{a.glyph}</span>
                  <span>
                    <span className="at">{a.title}</span>
                    <span className="ad">{a.desc}</span>
                  </span>
                </div>
              );
            })}
          </div>
        </TweakSection>

        <TweakSection label="Graveyard">
          <div className="graveyard-list">
            {state.graveyard.length === 0 && (
              <div style={{color:"var(--faint)", fontSize:"11px", fontStyle:"italic"}}>
                empty. your current pet still lives.
              </div>
            )}
            {state.graveyard.map((g, i) => (
              <div key={i} className="graveyard-row">
                <span className="gy-glyph">✦</span>
                <span className="gy-name">{g.name}</span>
                <span className="gy-cause">{g.cause}</span>
                <span className="gy-age">{g.age}</span>
              </div>
            ))}
          </div>
        </TweakSection>
      </TweaksPanel>
    </div>
  );
}

// ── Helpers ────────────────────────────────────────────────────────────────

// Cinematic full-screen splash shown when the pet evolves to a new stage.
function EvolutionOverlay({ pet, from, to }) {
  const fromFrame = petFrame(pet, from).join("\n");
  const toFrame = petFrame(pet, to).join("\n");
  return (
    <div className="evo-overlay">
      <div className="evo-card">
        <div className="evo-stars">✦  ✧  ✦  ✧  ✦  ✧  ✦  ✧  ✦</div>
        <div className="evo-headline">
          <span className="evo-pulse">{pet.name.toLowerCase()}</span>
          <span className="evo-evolved"> evolved!</span>
        </div>
        <div className="evo-pets">
          <div className="evo-pet">
            <pre className="evo-art" style={{ color: pet.color }}>{fromFrame}</pre>
            <div className="evo-stage-name">{stageLabel(pet, from)}</div>
          </div>
          <div className="evo-arrow">━━▶</div>
          <div className="evo-pet evo-pet-new">
            <pre className="evo-art" style={{ color: pet.color }}>{toFrame}</pre>
            <div className="evo-stage-name evo-stage-new">{stageLabel(pet, to)}</div>
          </div>
        </div>
        <div className="evo-stars">✧  ✦  ✧  ✦  ✧  ✦  ✧  ✦  ✧</div>
        <div className="evo-sub">stage <b>{from}</b> → <b>{to}</b> · keep shipping</div>
      </div>
    </div>
  );
}

function xpPercent(lifetime, pace = 100_000) {
  const cur = stageFromTokens(lifetime, pace);
  const idx = STAGES.indexOf(cur);
  if (idx === STAGES.length - 1) return 100;
  const STAGE_DAYS = [0, 0.04, 0.25, 1, 4, 14, 60];
  const lo = STAGE_DAYS[idx] * pace;
  const hi = STAGE_DAYS[idx + 1] * pace;
  return Math.min(100, Math.max(0, ((lifetime - lo) / (hi - lo)) * 100));
}

function parseTokenAmount(s) {
  const m = String(s).trim().toLowerCase().match(/^([\d.]+)\s*([km]?)$/);
  if (!m) return 47_000;
  const n = parseFloat(m[1]);
  if (m[2] === "m") return Math.round(n * 1_000_000);
  if (m[2] === "k") return Math.round(n * 1_000);
  return Math.round(n);
}

function pickBubble(arr) {
  return arr[Math.floor(Math.random() * arr.length)];
}

function moodLabel(m) {
  const map = {
    ecstatic: "ecstatic ✦",
    happy:    "happy ♡",
    content:  "content",
    hungry:   "hungry",
    sleepy:   "sleepy zzz",
    sad:      "sad",
  };
  return map[m] || m;
}

function initialLog() {
  return [
    { time: "12:14", text: "Mochi watched you commit auth.ts (+42 lines)", kind: "info" },
    { time: "12:42", text: "Mochi devoured 47k tokens (yum)", kind: "feed" },
    { time: "12:55", text: "PR #892 merged → +50 happy", kind: "ship" },
    { time: "13:18", text: "Mochi is doing great ♡", kind: "info" },
    { time: "13:42", text: "Mochi blinked at you", kind: "info" },
    { time: "13:51", text: "Mochi devoured 89k tokens (yum)", kind: "feed" },
    { time: "14:08", text: "Mochi noticed you've been quiet for 22 min", kind: "info" },
  ];
}

function ambientLines(stage, mood, name) {
  const base = [
    { text: `${name} blinked`, kind: "info" },
    { text: `${name} stretched`, kind: "info" },
    { text: `${name} glanced at the cursor`, kind: "info" },
    { text: `${name} hummed softly`, kind: "info" },
  ];
  if (mood === "hungry") base.push(
    { text: `${name} is hungry — burn more tokens!`, kind: "warn" },
    { text: `stomach rumble. ${name} stares at the prompt`, kind: "warn" },
  );
  if (mood === "sleepy") base.push(
    { text: `${name} yawned`, kind: "info" },
    { text: `${name} curled up. low activity detected.`, kind: "warn" },
  );
  if (mood === "sad") base.push(
    { text: `${name} looks a little down`, kind: "warn" },
  );
  if (mood === "happy" || mood === "content") base.push(
    { text: `${name} looks pleased with your work`, kind: "info" },
    { text: `${name} purred softly`, kind: "info" },
  );
  if (stage === "egg") base.push(
    { text: `the egg wobbled slightly`, kind: "info" },
    { text: `you hear a tiny tap from inside the shell`, kind: "info" },
  );
  if (stage === "sage") base.push(
    { text: `${name}'s aura shimmers ✦`, kind: "info" },
    { text: `${name} contemplates the codebase`, kind: "info" },
  );
  return base;
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
