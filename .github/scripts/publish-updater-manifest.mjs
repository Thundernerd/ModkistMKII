import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const UPDATER_TAG = "updater";
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

  const manifest = buildManifest(version, platforms);
  const prereleasePath = path.join(tmpDir, "latest-prerelease.json");
  fs.writeFileSync(prereleasePath, `${JSON.stringify(manifest, null, 2)}\n`);

  const uploadFiles = [prereleasePath];
  if (channel === "none") {
    const stablePath = path.join(tmpDir, "latest.json");
    fs.writeFileSync(stablePath, `${JSON.stringify(manifest, null, 2)}\n`);
    uploadFiles.push(stablePath);
  }

  ensureUpdaterRelease();
  gh(["release", "upload", UPDATER_TAG, ...uploadFiles, "--clobber"]);

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
