#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}
console.log("{not valid json");
