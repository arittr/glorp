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
})();
