import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptsDir, "../..");
const versionFiles = [
  "package.json",
  "package-lock.json",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
];

function copyVersionFiles() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "set-release-version-"));
  for (const file of versionFiles) {
    fs.mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), path.join(dir, file));
  }
  return dir;
}

test("sets the release version in package-lock.json", (t) => {
  const dir = copyVersionFiles();
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  execFileSync("node", [path.join(scriptsDir, "set-release-version.mjs")], {
    cwd: dir,
    env: { ...process.env, VERSION: "9.9.9" },
    stdio: "pipe",
  });

  const lock = JSON.parse(
    fs.readFileSync(path.join(dir, "package-lock.json"), "utf8"),
  );
  assert.equal(lock.version, "9.9.9");
  assert.equal(lock.packages[""].version, "9.9.9");
});
