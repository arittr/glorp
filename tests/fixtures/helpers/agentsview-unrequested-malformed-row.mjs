#!/usr/bin/env node
const args = process.argv.slice(2);

if (args.includes("--version")) {
  console.log("agentsview v0.32.1");
  process.exit(0);
}

const agent = args[args.indexOf("--agent") + 1];
const daily = agent === "claude"
  ? [
      {
        date: "2026-06-17",
        modelBreakdowns: [
          {
            modelName: "claude-opus-4-8",
            inputTokens: "not-a-number",
            outputTokens: 0,
            cacheCreationTokens: 0,
            cacheReadTokens: 0
          }
        ]
      }
    ]
  : [];

console.log(JSON.stringify({ daily }));
