#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.resolve(here, "..");
const args = process.argv.slice(2);

if (args.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}

if (!args.includes("--since")) {
  console.error("missing since");
  process.exit(2);
}
const since = args[args.indexOf("--since") + 1];
if (since !== "1970-01-01") {
  if (!args.includes("--until") || args[args.indexOf("--until") + 1] !== since) {
    console.error("missing current-day until");
    process.exit(2);
  }
}

const agent = args[args.indexOf("--agent") + 1];
const file = agent === "claude" ? "agentsview-claude-daily.json" : "agentsview-codex-daily.json";
const payload = JSON.parse(fs.readFileSync(path.join(fixtures, file), "utf8"));
payload.daily[0].modelBreakdowns[0].inputTokens += 100;
payload.daily[0].modelBreakdowns[0].outputTokens += 200;
payload.daily[0].modelBreakdowns[0].cacheCreationTokens += 300;
payload.daily[0].modelBreakdowns[0].cacheReadTokens += 400;
process.stdout.write(JSON.stringify(payload, null, 2));
