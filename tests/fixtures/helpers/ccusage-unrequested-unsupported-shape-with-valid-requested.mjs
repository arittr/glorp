#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-07-06",
      model: "claude-fable-5",
      inputTokens: 100,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      cost: 0.01
    },
    {
      date: "2026-07-05",
      model: "secret-model-project-name",
      inputTokens: 999999,
      outputTokens: 0
    }
  ]
}));
