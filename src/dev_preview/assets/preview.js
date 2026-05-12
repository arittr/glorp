(() => {
  const hash = window.location.hash.slice(1);
  if (!hash) return;

  for (const frame of document.querySelectorAll("[data-frame-id]")) {
    frame.hidden = frame.dataset.frameId !== hash;
  }
})();
