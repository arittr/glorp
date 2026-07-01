#!/usr/bin/env node
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("ccusage-codex 18.0.11");
  process.exit(0);
}
if (args[0] === "daily" && args.includes("--json") && args.includes("--offline")) {
  process.stdout.write(JSON.stringify({ daily: [] }));
  process.exit(0);
}
console.error(`unsupported ccusage-codex fixture args: ${args.join(" ")}`);
process.exit(2);
