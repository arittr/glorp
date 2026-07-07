#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({ daily: [] }));
