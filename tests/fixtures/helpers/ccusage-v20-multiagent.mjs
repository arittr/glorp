#!/usr/bin/env node
// Simulates ccusage >= 20: the bare `daily` command aggregates ALL agents
// (gpt/gemini rows, renamed `period` field), while the `claude daily`
// subcommand keeps the claude-only legacy shape (`date` field). glorp must
// invoke the subcommand on this version or it ingests non-claude history.
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
if (args[0] === "claude" && args[1] === "daily" && args.includes("--json")) {
  process.stdout.write(
    JSON.stringify({
      daily: [
        {
          date: "2026-06-10",
          inputTokens: 100,
          outputTokens: 200,
          cacheCreationTokens: 0,
          cacheReadTokens: 1000,
          totalCost: 0,
          modelsUsed: ["claude-fable-5"],
          modelBreakdowns: [
            {
              modelName: "claude-fable-5",
              inputTokens: 100,
              outputTokens: 200,
              cacheCreationTokens: 0,
              cacheReadTokens: 1000,
              cost: 0,
            },
          ],
        },
      ],
      totals: {},
    }),
  );
  process.exit(0);
}
if (args[0] === "daily" && args.includes("--json")) {
  // The all-agents aggregate a wrong invocation would ingest.
  process.stdout.write(
    JSON.stringify({
      daily: [
        {
          period: "2026-06-10",
          agent: "all",
          inputTokens: 999999,
          outputTokens: 999999,
          cacheCreationTokens: 999999,
          cacheReadTokens: 999999,
          totalCost: 42,
          modelsUsed: ["gpt-5.5", "claude-fable-5"],
          modelBreakdowns: [
            {
              modelName: "gpt-5.5",
              inputTokens: 999999,
              outputTokens: 999999,
              cacheCreationTokens: 0,
              cacheReadTokens: 0,
              cost: 21,
            },
            {
              modelName: "claude-fable-5",
              inputTokens: 100,
              outputTokens: 200,
              cacheCreationTokens: 0,
              cacheReadTokens: 1000,
              cost: 0,
            },
          ],
        },
      ],
      totals: {},
    }),
  );
  process.exit(0);
}
console.error(`unsupported ccusage fixture args: ${args.join(" ")}`);
process.exit(2);
