import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const bin = path.resolve(here, "../bin/glorp.js");
const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "glorp-npm-smoke-"));
const fakeNative = path.join(tempRoot, process.platform === "win32" ? "glorp.cmd" : "glorp");
const envLog = path.join(tempRoot, "native-env.json");
const nodePath = path.join(tempRoot, "node_modules");

if (process.platform === "win32") {
  fs.writeFileSync(fakeNative, `@"${process.execPath}" "${path.join(tempRoot, "fake-native.mjs")}" %*\r\n`);
} else {
  fs.writeFileSync(fakeNative, `#!/bin/sh\nexec "${process.execPath}" "${path.join(tempRoot, "fake-native.mjs")}" "$@"\n`);
  fs.chmodSync(fakeNative, 0o755);
}

fs.writeFileSync(
  path.join(tempRoot, "fake-native.mjs"),
  `import fs from "node:fs";
const cmd = process.argv[2] || "help";
fs.writeFileSync(${JSON.stringify(envLog)}, JSON.stringify({
  agentsview: process.env.GLORP_AGENTSVIEW_BIN,
  ccusage: process.env.GLORP_CCUSAGE_BIN,
  codex: process.env.GLORP_CCUSAGE_CODEX_BIN,
  node: process.env.GLORP_NODE_BIN,
  skip: process.env.GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST,
  companionApp: process.env.GLORP_COMPANION_APP,
  path: process.env.PATH
}, null, 2));
if (cmd === "help") {
  console.log("glorp init watch status doctor reset rename");
  process.exit(0);
}
if (cmd === "feed") {
  console.error("manual feed is not supported");
  process.exit(2);
}
if (cmd === "doctor") {
  if (process.env.GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST === "1" && !process.env.PATH.includes("glorp-path-helper-")) {
    console.log("ccusage helper not found; setup blocked");
  } else {
    console.log("ccusage helper ok");
  }
  process.exit(0);
}
console.log("ok");
`
);

function writeHelperPackage(name, binName) {
  const dir = path.join(nodePath, ...name.split("/"));
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "package.json"),
    JSON.stringify(
      {
        name,
        version: "0.0.0-smoke",
        bin: {
          [binName]: "bin/helper.js"
        }
      },
      null,
      2
    )
  );
  fs.mkdirSync(path.join(dir, "bin"), { recursive: true });
  fs.writeFileSync(path.join(dir, "bin/helper.js"), "#!/usr/bin/env node\n");
}

writeHelperPackage("ccusage", "ccusage");
writeHelperPackage("@ccusage/codex", "ccusage-codex");

function run(args, env = {}) {
  return spawnSync(process.execPath, [bin, ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      GLORP_NATIVE_BIN_FOR_TEST: fakeNative,
      NODE_PATH: nodePath,
      ...env
    }
  });
}

const help = run(["help"]);
assert.equal(help.status, 0, help.stderr);
assert.match(help.stdout, /watch/);
assert.doesNotMatch(help.stdout, /\bfeed\b/);

const manualFeed = run(["feed"]);
assert.notEqual(manualFeed.status, 0);

const doctor = run(["doctor"], {
  GLORP_CONFIG_DIR: path.join(tempRoot, "config")
});
assert.equal(doctor.status, 0, doctor.stderr);
assert.match(doctor.stdout, /ccusage|helper/i);
assert.doesNotMatch(doctor.stdout, /secret prompt|secret response|tool payload/);
const bundledEnv = JSON.parse(fs.readFileSync(envLog, "utf8"));
assert.equal(bundledEnv.node, process.execPath);
assert.ok(fs.existsSync(bundledEnv.ccusage), bundledEnv.ccusage);
assert.ok(fs.existsSync(bundledEnv.codex), bundledEnv.codex);
assert.match(bundledEnv.ccusage.replaceAll("\\", "/"), /node_modules\/ccusage\//);
assert.match(bundledEnv.codex.replaceAll("\\", "/"), /node_modules\/@ccusage\/codex\//);

const tempBin = fs.mkdtempSync(path.join(os.tmpdir(), "glorp-path-helper-"));
const pathShim = path.join(tempBin, process.platform === "win32" ? "ccusage.cmd" : "ccusage");
if (process.platform === "win32") {
  fs.writeFileSync(pathShim, "@echo ccusage fixture\r\n");
} else {
  fs.writeFileSync(pathShim, "#!/bin/sh\necho ccusage fixture\n");
  fs.chmodSync(pathShim, 0o755);
}

const fallback = run(["doctor"], {
  GLORP_CONFIG_DIR: path.join(tempRoot, "config-path-fallback"),
  GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST: "1",
  PATH: tempBin
});
assert.equal(fallback.status, 0, fallback.stderr);
assert.match(fallback.stdout, /ccusage/i);
assert.doesNotMatch(fallback.stdout, /not found/i);
const fallbackEnv = JSON.parse(fs.readFileSync(envLog, "utf8"));
assert.equal(fallbackEnv.skip, "1");
assert.equal(fallbackEnv.ccusage, undefined);
assert.equal(fallbackEnv.codex, undefined);

const missing = run(["doctor"], {
  GLORP_CONFIG_DIR: path.join(tempRoot, "config-missing"),
  GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST: "1",
  PATH: fs.mkdtempSync(path.join(os.tmpdir(), "glorp-empty-path-"))
});
assert.equal(missing.status, 0, missing.stderr);
assert.match(missing.stdout, /not found|No usage helper|blocked/i);

const externalAgentsview = path.join(tempRoot, process.platform === "win32" ? "agentsview.cmd" : "agentsview");
if (process.platform === "win32") {
  fs.writeFileSync(externalAgentsview, "@echo agentsview fixture\r\n");
} else {
  fs.writeFileSync(externalAgentsview, "#!/bin/sh\necho agentsview fixture\n");
  fs.chmodSync(externalAgentsview, 0o755);
}

const withAgentsview = run(["doctor"], {
  GLORP_AGENTSVIEW_BIN: externalAgentsview,
  GLORP_CONFIG_DIR: path.join(tempRoot, "config-agentsview")
});
assert.equal(withAgentsview.status, 0, withAgentsview.stderr);
const agentsviewEnv = JSON.parse(fs.readFileSync(envLog, "utf8"));
assert.equal(agentsviewEnv.agentsview, externalAgentsview);

const fakeApp = path.join(tempRoot, "Glorp.app");
fs.mkdirSync(fakeApp, { recursive: true });
const companion = run(["companion"], {
  GLORP_COMPANION_APP_FOR_TEST: fakeApp
});
assert.equal(companion.status, 0, companion.stderr);
const companionEnv = JSON.parse(fs.readFileSync(envLog, "utf8"));
assert.equal(companionEnv.companionApp, fakeApp);
