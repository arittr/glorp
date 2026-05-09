#!/usr/bin/env node
process.env.CCUSAGE_FIXTURE = "ccusage-daily-next.json";
await import("./ccusage-ok.mjs");
