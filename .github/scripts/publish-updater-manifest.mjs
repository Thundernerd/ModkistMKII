import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const UPDATER_TAG = "updater";
const TEMP_SUFFIX = ".next";
const REQUIRED_PLATFORMS = [
  "darwin-aarch64",
  "darwin-x86_64",
  "windows-x86_64",
  "linux-x86_64",
];

function requiredEnv(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is not set`);
  }
  return value;
}

function ghJson(args) {
  return JSON.parse(execFileSync("gh", args, { encoding: "utf8" }));
}

function gh(args, options = {}) {
  execFileSync("gh", args, { stdio: "inherit", ...options });
}

function ghOutput(args) {
  return execFileSync("gh", args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
}

export function assetUrl(repo, tag, name) {
  return `https://github.com/${repo}/releases/download/${tag}/${name}`;
}

export function findAsset(names, predicate) {
  return names.find(predicate);
}

export function buildPlatforms(assetNames, signatures, repo, tag) {
  const platforms = {};

  const appTar = findAsset(
    assetNames,
    (name) => name.endsWith(".app.tar.gz") && !name.endsWith(".sig"),
  );
  if (appTar) {
    const signature = signatures[`${appTar}.sig`];
    if (!signature) {
      throw new Error(`Missing signature for ${appTar}`);
    }
    const url = assetUrl(repo, tag, appTar);
    platforms["darwin-aarch64"] = { signature, url };
    platforms["darwin-x86_64"] = { signature, url };
  }

  const msi = findAsset(
    assetNames,
    (name) => name.endsWith(".msi") && !name.endsWith(".sig"),
  );
  if (msi) {
    const signature = signatures[`${msi}.sig`];
    if (!signature) {
      throw new Error(`Missing signature for ${msi}`);
    }
    platforms["windows-x86_64"] = {
      signature,
      url: assetUrl(repo, tag, msi),
    };
  }

  const appImage = findAsset(
    assetNames,
    (name) => name.endsWith(".AppImage") && !name.endsWith(".sig"),
  );
  if (appImage) {
    const signature = signatures[`${appImage}.sig`];
    if (!signature) {
      throw new Error(`Missing signature for ${appImage}`);
    }
    platforms["linux-x86_64"] = {
      signature,
      url: assetUrl(repo, tag, appImage),
    };
  }

  return platforms;
}

export function buildManifest(version, platforms, pubDate = new Date()) {
  return {
    version,
    pub_date: pubDate.toISOString(),
    platforms,
  };
}

function tempName(name) {
  return `${name}${TEMP_SUFFIX}`;
}

function listAssets(runGh, repo, tag) {
  const release = JSON.parse(
    runGh(["api", `repos/${repo}/releases/tags/${tag}`]),
  );
  return release.assets ?? [];
}

function deleteAsset(runGh, repo, id) {
  runGh(["api", "-X", "DELETE", `repos/${repo}/releases/assets/${id}`]);
}

function renameAsset(runGh, repo, id, name) {
  runGh([
    "api",
    "-X",
    "PATCH",
    `repos/${repo}/releases/assets/${id}`,
    "-f",
    `name=${name}`,
  ]);
}

/**
 * Replaces updater manifests on the release `tag` without a window in which
 * the old manifest is gone and the new one is not yet uploaded.
 *
 * Each manifest is uploaded as `<name>.next`. Only once every upload has
 * succeeded is the old asset deleted and the temp asset renamed to `<name>`.
 * A failed upload throws before anything is deleted, so the previous manifest
 * stays downloadable.
 *
 * `runGh` runs the gh CLI with the given arguments and returns its stdout.
 */
export function replaceManifests({ runGh, repo, tag, dir, manifests }) {
  const tempNames = new Set(manifests.map(({ name }) => tempName(name)));
  for (const stale of listAssets(runGh, repo, tag)) {
    if (tempNames.has(stale.name)) {
      deleteAsset(runGh, repo, stale.id);
    }
  }

  for (const { name, content } of manifests) {
    const file = path.join(dir, tempName(name));
    fs.writeFileSync(file, content);
    runGh(["release", "upload", tag, file]);
  }

  const assets = listAssets(runGh, repo, tag);
  for (const { name } of manifests) {
    const old = assets.find((asset) => asset.name === name);
    const next = assets.find((asset) => asset.name === tempName(name));
    if (!next) {
      throw new Error(
        `Uploaded ${tempName(name)} is missing from release ${tag}`,
      );
    }
    if (old) {
      deleteAsset(runGh, repo, old.id);
    }
    renameAsset(runGh, repo, next.id, name);
  }
}

function ensureUpdaterRelease() {
  try {
    gh(["release", "view", UPDATER_TAG], { stdio: "ignore" });
  } catch {
    gh([
      "release",
      "create",
      UPDATER_TAG,
      "--title",
      "Updater manifests",
      "--notes",
      "Rolling updater manifests for Modkist. This is not an app release.",
      "--prerelease",
    ]);
  }
}

function main() {
  const version = requiredEnv("VERSION");
  const tag = requiredEnv("TAG");
  const channel = requiredEnv("PRERELEASE_CHANNEL");
  const repo = requiredEnv("GITHUB_REPOSITORY");

  const release = ghJson(["release", "view", tag, "--json", "assets"]);
  const assetNames = (release.assets ?? []).map((asset) => asset.name);
  if (assetNames.length === 0) {
    throw new Error(`No assets found on release ${tag}`);
  }

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "modkist-updater-"));
  gh(["release", "download", tag, "--pattern", "*.sig", "--dir", tmpDir]);

  const signatures = {};
  for (const file of fs.readdirSync(tmpDir)) {
    if (!file.endsWith(".sig")) {
      continue;
    }
    signatures[file] = fs.readFileSync(path.join(tmpDir, file), "utf8").trim();
  }

  const platforms = buildPlatforms(assetNames, signatures, repo, tag);
  const missing = REQUIRED_PLATFORMS.filter((key) => !platforms[key]);
  if (missing.length > 0) {
    throw new Error(
      `Updater manifest is missing platforms: ${missing.join(", ")}`,
    );
  }

  const content = `${JSON.stringify(buildManifest(version, platforms), null, 2)}\n`;
  const manifests = [{ name: "latest-prerelease.json", content }];
  if (channel === "none") {
    manifests.push({ name: "latest.json", content });
  }

  ensureUpdaterRelease();
  replaceManifests({
    runGh: ghOutput,
    repo,
    tag: UPDATER_TAG,
    dir: tmpDir,
    manifests,
  });

  console.error(
    `Published updater manifest for ${version} (${channel === "none" ? "stable + prerelease" : "prerelease only"})`,
  );
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(`::error::${error instanceof Error ? error.message : error}`);
    process.exit(1);
  }
}
