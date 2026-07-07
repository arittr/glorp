#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-07-06",
      model: "claude-renamed",
      inputTokens: 531,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0
    }
  ]
}));
