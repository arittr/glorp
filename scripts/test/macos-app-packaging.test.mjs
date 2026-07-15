import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  macosCargoBuildArgs,
  readAppVersion,
  resolveMacosBinaryPath,
} from "../build-macos-app-shared.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readPublishWorkflow() {
  return fs.readFileSync(
    path.join(repoRoot, ".github/workflows/publish.yml"),
    "utf8",
  );
}

// The publish matrix block for one target, from its `- target:` line up to the
// next target (or the end of the matrix `include:` list), so per-target
// assertions cannot leak across targets or into the reusable step definitions.
function targetMatrixBlock(workflow, target, nextTarget) {
  const matrixSection = workflow.slice(
    workflow.indexOf("include:"),
    workflow.indexOf("runs-on: ${{ matrix.runner }}"),
  );
  const start = matrixSection.indexOf(`- target: ${target}`);
  const end = nextTarget
    ? matrixSection.indexOf(`- target: ${nextTarget}`)
    : matrixSection.length;
  return matrixSection.slice(start, end);
}

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

  it("appends the requested cargo features to a local companion build", () => {
    assert.deepEqual(
      macosCargoBuildArgs({ profile: "release", features: ["retained-renderer"] }),
      ["build", "--bin", "glorp", "--release", "--features", "retained-renderer"],
    );
    assert.deepEqual(macosCargoBuildArgs({ profile: "release" }), [
      "build",
      "--bin",
      "glorp",
      "--release",
    ]);
  });

  it("ships the retained renderer on Apple Silicon and Smooth-only on Intel", () => {
    const workflow = readPublishWorkflow();

    // The release build consumes the per-target feature set.
    assert.match(
      workflow,
      /cargo build --release --locked \$\{\{ matrix\.cargo_features \}\} --target \$\{\{ matrix\.rust_target \}\}/,
    );

    const armBlock = targetMatrixBlock(workflow, "darwin-arm64", "darwin-x64");
    assert.match(
      armBlock,
      /cargo_features: --no-default-features --features retained-renderer/,
    );

    const intelBlock = targetMatrixBlock(workflow, "darwin-x64", "linux-x64");
    assert.match(intelBlock, /cargo_features: --no-default-features\b/);
    assert.doesNotMatch(intelBlock, /retained-renderer/);

    // Non-macOS targets keep the unchanged no-default build.
    for (const [target, next] of [
      ["linux-x64", "linux-arm64"],
      ["linux-arm64", "win32-x64"],
      ["win32-x64", null],
    ]) {
      const block = targetMatrixBlock(workflow, target, next);
      assert.match(block, /cargo_features: --no-default-features\b/);
      assert.doesNotMatch(block, /retained-renderer/);
    }
  });

  it("runs staged capability smokes proving the shipped matrix before upload", () => {
    const workflow = readPublishWorkflow();

    // Apple Silicon: use the exact app-bundle executable on a native logged-in
    // host. Auto stays on the legacy route while the Gate D rollback is active;
    // explicit Live still proves and captures the direct path.
    const armSmoke = "staged direct route proof (Apple Silicon)";
    assert(workflow.includes(armSmoke));
    const armSmokeBlock = workflow.slice(
      workflow.indexOf(armSmoke),
      workflow.indexOf("staged smooth-only smoke (Intel)"),
    );
    assert.match(armSmokeBlock, /companion-app --print-capabilities/);
    assert.match(armSmokeBlock, /retained-compiled=true/);
    assert.match(armSmokeBlock, /auto-retained-on-apple-silicon=true/);
    assert.match(armSmokeBlock, /effective-renderer=retained/);
    assert.match(armSmokeBlock, /effective-scene-route=legacy/);
    assert.match(
      armSmokeBlock,
      /companion-app --renderer retained --retained-scene-runtime live --print-capabilities/,
    );
    assert.match(armSmokeBlock, /effective-scene-route=direct/);
    assert.match(
      armSmokeBlock,
      /app_bin="target\/macos\/Glorp\.app\/Contents\/MacOS\/glorp"/,
    );
    assert.match(armSmokeBlock, /test "\$\(uname -m\)" = "arm64"/);
    assert.match(armSmokeBlock, /stat -f '%Su' \/dev\/console/);
    assert.match(armSmokeBlock, /test "\$\(id -un\)" = "\$console_user"/);
    assert.match(
      armSmokeBlock,
      /capture_path="\$capture_dir\/scene\.png"/,
    );
    assert.match(
      armSmokeBlock,
      /--renderer retained \\\n+\s+--retained-scene-runtime live \\\n+\s+--review-size 360x360/,
    );
    assert.match(armSmokeBlock, /test -f "\$capture_path"/);
    assert.match(armSmokeBlock, /for artifact in scene-manifest\.json scene-snapshot\.json scene-version\.json scene-metrics\.json scene-artifacts\.json/);
    assert.match(armSmokeBlock, /test -s "\$capture_dir\/\$artifact"/);
    assert.match(armSmokeBlock, /direct-retained-scene/);
    assert.match(armSmokeBlock, /nonblank_validation/);
    assert.match(armSmokeBlock, /CAPTURE_PATH="\$capture_path" swift/);
    assert.match(armSmokeBlock, /visibleNonblack/);
    assert.match(armSmokeBlock, /colors\.count > 1/);

    // Intel: retained unavailable, so Auto stays Smooth even though the cutover
    // constant is on; explicit Retained is rejected.
    const intelSmoke = "staged smooth-only smoke (Intel)";
    assert(workflow.includes(intelSmoke));
    const intelSmokeBlock = workflow.slice(
      workflow.indexOf(intelSmoke),
      workflow.indexOf("stage binary at workspace root for upload"),
    );
    assert.match(intelSmokeBlock, /retained-compiled=false/);
    assert.match(intelSmokeBlock, /auto-retained-on-apple-silicon=true/);
    assert.match(intelSmokeBlock, /effective-renderer=smooth/);
    assert.match(
      intelSmokeBlock,
      /if "\$bin" companion-app --renderer retained --print-capabilities; then/,
    );

    // The smokes must gate the upload: both run before the artifact is staged
    // for upload.
    assert(
      workflow.indexOf(armSmoke) <
        workflow.indexOf("stage binary at workspace root for upload"),
    );
    assert(
      workflow.indexOf(intelSmoke) <
        workflow.indexOf("stage binary at workspace root for upload"),
    );
  });
});
