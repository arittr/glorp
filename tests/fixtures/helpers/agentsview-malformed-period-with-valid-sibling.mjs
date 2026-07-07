#!/usr/bin/env node
const args = process.argv.slice(2);

if (args.includes("--version")) {
  console.log("agentsview v0.32.1");
  process.exit(0);
}

if (!args.includes("usage") || !args.includes("daily") || !args.includes("--json")) {
  console.error("unexpected agentsview argv");
  process.exit(2);
}
if (!args.includes("--timezone") || args[args.indexOf("--timezone") + 1] !== "America/Los_Angeles") {
  console.error("missing timezone");
  process.exit(2);
}

const agent = args[args.indexOf("--agent") + 1];
console.log(JSON.stringify({
  daily: [
    {
      date: "2026/06/18",
      modelBreakdowns: [
        {
          modelName: `${agent}-bad-period`,
          inputTokens: 100,
          outputTokens: 0,
          cacheCreationTokens: 0,
          cacheReadTokens: 0
        }
      ]
    },
    {
      date: "2026-06-18",
      modelBreakdowns: [
        {
          modelName: `${agent}-valid`,
          inputTokens: 100,
          outputTokens: 0,
          cacheCreationTokens: 0,
          cacheReadTokens: 0
        }
      ]
    }
  ]
}));
