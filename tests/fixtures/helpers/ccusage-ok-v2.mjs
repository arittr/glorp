#!/usr/bin/env node
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("ccusage 18.0.99");
  process.exit(0);
}
await import("./ccusage-ok.mjs");
