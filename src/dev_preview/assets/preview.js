(() => {
  const hash = window.location.hash.slice(1);
  if (hash) {
    for (const frame of document.querySelectorAll("[data-frame-id]")) {
      frame.hidden = frame.dataset.frameId !== hash;
    }
  }

  const toggles = Array.from(document.querySelectorAll("[data-overlay-toggle]"));
  const overlays = Array.from(document.querySelectorAll(".layout-overlay"));

  const syncOverlays = () => {
    const activeKinds = new Set(
      toggles
        .filter((toggle) => toggle.getAttribute("aria-pressed") === "true")
        .map((toggle) => toggle.dataset.overlayToggle),
    );

    for (const overlay of overlays) {
      overlay.hidden = activeKinds.size === 0;
      overlay.classList.toggle("show-components", activeKinds.has("components"));
      overlay.classList.toggle("show-targets", activeKinds.has("targets"));
    }
  };

  for (const toggle of toggles) {
    toggle.addEventListener("click", () => {
      const pressed = toggle.getAttribute("aria-pressed") === "true";
      toggle.setAttribute("aria-pressed", String(!pressed));
      syncOverlays();
    });
  }

  syncOverlays();

  const stripTimers = new Map();

  const showStripFrame = (strip, index) => {
    const frames = Array.from(strip.querySelectorAll("[data-strip-frame]"));
    const count = frames.length;
    const nextIndex = ((index % count) + count) % count;
    strip.dataset.frameIndex = String(nextIndex);
    frames.forEach((frame, frameIndex) => {
      frame.hidden = frameIndex !== nextIndex;
    });
  };

  const pauseStrip = (strip) => {
    const timer = stripTimers.get(strip);
    if (timer) {
      window.clearInterval(timer);
      stripTimers.delete(strip);
    }
    const play = strip.querySelector("[data-strip-play]");
    if (play) {
      play.setAttribute("aria-pressed", "false");
      play.textContent = "Play";
    }
  };

  for (const strip of document.querySelectorAll("[data-strip-id]")) {
    const play = strip.querySelector("[data-strip-play]");
    const prev = strip.querySelector("[data-strip-prev]");
    const next = strip.querySelector("[data-strip-next]");
    showStripFrame(strip, 0);

    play?.addEventListener("click", () => {
      const pressed = play.getAttribute("aria-pressed") === "true";
      if (pressed) {
        pauseStrip(strip);
        return;
      }
      play.setAttribute("aria-pressed", "true");
      play.textContent = "Pause";
      const duration = Number(strip.dataset.frameDuration || "160");
      const timer = window.setInterval(() => {
        showStripFrame(strip, Number(strip.dataset.frameIndex || "0") + 1);
      }, duration);
      stripTimers.set(strip, timer);
    });

    prev?.addEventListener("click", () => {
      pauseStrip(strip);
      showStripFrame(strip, Number(strip.dataset.frameIndex || "0") - 1);
    });

    next?.addEventListener("click", () => {
      pauseStrip(strip);
      showStripFrame(strip, Number(strip.dataset.frameIndex || "0") + 1);
    });
  }
})();
