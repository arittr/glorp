import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  readAppVersion,
  resolveMacosBinaryPath,
} from "../build-macos-app-shared.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
}

describe("macOS app packaging", () => {
  it("versions app bundles from the published npm package", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "glorp-macos-app-version-"));
    writeJson(path.join(root, "package.json"), { private: true });
    writeJson(path.join(root, "npm/glorp/package.json"), {
      name: "@arittr/glorp",
      version: "1.2.3",
    });

    assert.equal(readAppVersion(root), "1.2.3");
  });

  it("can package an already-built target binary instead of the host binary", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "glorp-macos-binary-path-"));
    const targetBinary = path.join(root, "target/x86_64-apple-darwin/release/glorp");

    assert.equal(
      resolveMacosBinaryPath({
        repoRoot: root,
        profile: "release",
        binary: targetBinary,
      }),
      targetBinary,
    );
    assert.equal(
      resolveMacosBinaryPath({ repoRoot: root, profile: "release" }),
      path.join(root, "target/release/glorp"),
    );
  });

  it("passes Darwin companion app artifacts from build to publish packaging", () => {
    const workflow = fs.readFileSync(
      path.join(repoRoot, ".github/workflows/publish.yml"),
      "utf8",
    );

    assert.match(
      workflow,
      /node scripts\/build-macos-companion-app\.mjs --binary "target\/\$\{\{ matrix\.rust_target \}\}\/release\/glorp"/,
    );
    assert.match(
      workflow,
      /tar -czf "glorp-app-\$\{\{ matrix\.target \}\}\.tgz" -C target\/macos Glorp\.app/,
    );
    assert.match(workflow, /name: glorp-app-\$\{\{ matrix\.target \}\}/);
    assert.match(
      workflow,
      /tar -xzf "target\/macos\/glorp-app-\$\{\{ matrix\.target \}\}\.tgz" -C target\/macos/,
    );

    assert(
      workflow.indexOf("build companion app bundle") <
        workflow.indexOf("upload companion app artifact"),
    );
    assert(
      workflow.indexOf("download companion app artifact") <
        workflow.indexOf("stage binary into platform package"),
    );
  });
});
