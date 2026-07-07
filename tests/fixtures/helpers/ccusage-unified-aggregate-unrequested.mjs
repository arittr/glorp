#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 22.0.0");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-07-04",
      model: "claude-fable-5",
      inputTokens: 999999,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      cost: 0.01
    }
  ]
}));
