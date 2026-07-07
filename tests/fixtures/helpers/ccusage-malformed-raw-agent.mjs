#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-07-06",
      agent: "/Users/drew/private/project-secret",
      model: "secret-model-project-name",
      inputTokens: "not-a-number",
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0
    }
  ]
}));
