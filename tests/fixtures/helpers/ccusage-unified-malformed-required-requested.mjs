#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 22.0.0");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-05-09",
      agent: "claude",
      model: "claude-fable-5",
      inputTokens: "not-a-number",
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0
    }
  ]
}));
