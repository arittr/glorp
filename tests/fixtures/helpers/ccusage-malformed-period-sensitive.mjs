#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "/Users/drew/private/session.jsonl secret transcript path",
      model: "secret-model-project-name",
      inputTokens: 100,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0
    }
  ]
}));
