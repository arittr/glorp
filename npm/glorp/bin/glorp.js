#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));

function platformKey() {
  const arch = process.arch === "x64" ? "x64" : process.arch === "arm64" ? "arm64" : process.arch;
  if (process.platform === "darwin") return `darwin-${arch}`;
  if (process.platform === "linux") return `linux-${arch}`;
  if (process.platform === "win32") return `win32-${arch}`;
  throw new Error(`unsupported platform: ${process.platform}-${process.arch}`);
}

function platformPackageName() {
  return `@arittr/glorp-${platformKey()}`;
}

function resolvePackageJson(pkg) {
  try {
    return require.resolve(`${pkg}/package.json`);
  } catch {
    if (!pkg.startsWith("@arittr/glorp-")) return undefined;
    const local = path.resolve(here, "../../platform", platformKey(), "package.json");
    return fs.existsSync(local) ? local : undefined;
  }
}

function resolveNativeBinary(env) {
  if (env.GLORP_NATIVE_BIN_FOR_TEST) return env.GLORP_NATIVE_BIN_FOR_TEST;

  const pkgJson = resolvePackageJson(platformPackageName());
  if (!pkgJson) throw new Error(`native glorp package missing for ${process.platform}-${process.arch}`);

  const exe = process.platform === "win32" ? "glorp.exe" : "glorp";
  const bin = path.join(path.dirname(pkgJson), "bin", exe);
  if (!fs.existsSync(bin)) throw new Error(`native glorp binary missing at ${bin}`);
  return bin;
}

function resolvePackageBin(pkg, binName) {
  const pkgJsonPath = resolvePackageJson(pkg);
  if (!pkgJsonPath) return undefined;

  try {
    const pkgJson = JSON.parse(fs.readFileSync(pkgJsonPath, "utf8"));
    const rel = typeof pkgJson.bin === "string" ? pkgJson.bin : pkgJson.bin?.[binName];
    return rel ? path.resolve(path.dirname(pkgJsonPath), rel) : undefined;
  } catch {
    return undefined;
  }
}

function resolveCompanionApp(env) {
  if (env.GLORP_COMPANION_APP_FOR_TEST) return env.GLORP_COMPANION_APP_FOR_TEST;
  if (process.platform !== "darwin") return undefined;
  const pkgJson = resolvePackageJson(platformPackageName());
  if (!pkgJson) return undefined;
  const app = path.join(path.dirname(pkgJson), "app", "Glorp.app");
  return fs.existsSync(app) ? app : undefined;
}

const env = { ...process.env };
if (env.GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST !== "1") {
  env.GLORP_CCUSAGE_BIN ??= resolvePackageBin("ccusage", "ccusage");
  env.GLORP_CCUSAGE_CODEX_BIN ??= resolvePackageBin("@ccusage/codex", "ccusage-codex");
}
env.GLORP_NODE_BIN ??= process.execPath;

const companionApp = resolveCompanionApp(env);
if (companionApp) env.GLORP_COMPANION_APP ??= companionApp;

let native;
try {
  native = resolveNativeBinary(env);
} catch (err) {
  console.error(`glorp: ${err.message}`);
  process.exit(1);
}

const child = spawnSync(native, process.argv.slice(2), { stdio: "inherit", env });
if (child.error) {
  console.error(`glorp: ${child.error.message}`);
  process.exit(1);
}
process.exit(child.status ?? 1);
