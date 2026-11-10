import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  assetUrl,
  buildManifest,
  buildPlatforms,
  replaceManifests,
} from "./publish-updater-manifest.mjs";

test("maps signed installer assets onto updater platforms", () => {
  const names = [
    "Modkist.app.tar.gz",
    "Modkist.app.tar.gz.sig",
    "Modkist_1.0.0-rc.9_x64_en-US.msi",
    "Modkist_1.0.0-rc.9_x64_en-US.msi.sig",
    "Modkist_1.0.0-rc.9_amd64.AppImage",
    "Modkist_1.0.0-rc.9_amd64.AppImage.sig",
    "Modkist_1.0.0-rc.9_universal.dmg",
  ];
  const signatures = {
    "Modkist.app.tar.gz.sig": "darwin-sig",
    "Modkist_1.0.0-rc.9_x64_en-US.msi.sig": "windows-sig",
    "Modkist_1.0.0-rc.9_amd64.AppImage.sig": "linux-sig",
  };

  const platforms = buildPlatforms(
    names,
    signatures,
    "Thundernerd/ModkistMKII",
    "v1.0.0-rc.9",
  );

  assert.deepEqual(platforms["darwin-aarch64"], {
    signature: "darwin-sig",
    url: assetUrl(
      "Thundernerd/ModkistMKII",
      "v1.0.0-rc.9",
      "Modkist.app.tar.gz",
    ),
  });
  assert.deepEqual(platforms["darwin-x86_64"], platforms["darwin-aarch64"]);
  assert.equal(platforms["windows-x86_64"].signature, "windows-sig");
  assert.equal(platforms["linux-x86_64"].signature, "linux-sig");
});

test("fails when a signature is missing", () => {
  assert.throws(
    () =>
      buildPlatforms(
        ["Modkist.app.tar.gz"],
        {},
        "Thundernerd/ModkistMKII",
        "v1.0.0",
      ),
    /Missing signature/,
  );
});

test("writes version and platform map into the manifest", () => {
  const pubDate = new Date("2026-08-25T12:00:00.000Z");
  const manifest = buildManifest(
    "1.0.0-rc.9",
    { "linux-x86_64": { signature: "sig", url: "https://example.test" } },
    pubDate,
  );

  assert.equal(manifest.version, "1.0.0-rc.9");
  assert.equal(manifest.pub_date, "2026-08-25T12:00:00.000Z");
  assert.equal(manifest.platforms["linux-x86_64"].signature, "sig");
});

const REPO = "Thundernerd/ModkistMKII";

// Reads a gh argv as the operation it performs on the release's assets.
function parseGhCall(args) {
  const [cmd, sub] = args;
  if (cmd === "release" && sub === "upload") {
    return { kind: "upload", file: args[3], name: path.basename(args[3]) };
  }
  if (cmd === "api" && args.includes("-X")) {
    const method = args[args.indexOf("-X") + 1];
    const id = Number(
      args
        .find((a) => a.includes("/assets/"))
        .split("/")
        .pop(),
    );
    if (method === "PATCH") {
      const name = args[args.indexOf("-f") + 1].replace(/^name=/, "");
      return { kind: "rename", id, name };
    }
    return { kind: "delete", id };
  }
  if (cmd === "api") {
    return { kind: "list" };
  }
  throw new Error(`unexpected gh call: ${args.join(" ")}`);
}

function describeCall(args) {
  const call = parseGhCall(args);
  switch (call.kind) {
    case "upload":
      return `upload ${call.name}`;
    case "rename":
      return `rename ${call.id} -> ${call.name}`;
    case "delete":
      return `delete ${call.id}`;
    default:
      return call.kind;
  }
}

// A fake `gh` runner that keeps the updater release's assets in memory and
// records every call, so tests can check both the order of operations and
// what a client would be able to download afterwards.
function fakeGh({ assets = [], failUploadOf = [] } = {}) {
  let nextId = 1000;
  const state = new Map(assets.map((asset) => [asset.name, { ...asset }]));
  const calls = [];
  const findById = (id) => {
    const entry = [...state.values()].find((asset) => asset.id === id);
    if (!entry) {
      throw new Error(`no asset ${id}`);
    }
    return entry;
  };
  const run = (args) => {
    calls.push(args);
    const call = parseGhCall(args);
    switch (call.kind) {
      case "list":
        return JSON.stringify({
          assets: [...state.values()].map(({ id, name }) => ({ id, name })),
        });
      case "upload":
        if (failUploadOf.includes(call.name)) {
          throw new Error(`upload of ${call.name} failed`);
        }
        if (state.has(call.name)) {
          throw new Error(`asset ${call.name} already exists`);
        }
        state.set(call.name, {
          id: nextId++,
          name: call.name,
          content: fs.readFileSync(call.file, "utf8"),
        });
        return "";
      case "delete":
        state.delete(findById(call.id).name);
        return "";
      case "rename": {
        const entry = findById(call.id);
        state.delete(entry.name);
        state.set(call.name, { ...entry, name: call.name });
        return "";
      }
    }
  };
  return { run, calls, state };
}

function tempDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "modkist-replace-test-"));
}

test("replaces a manifest by uploading a temp asset, then deleting the old one, then renaming", () => {
  const gh = fakeGh({
    assets: [{ id: 1, name: "latest-prerelease.json", content: "old" }],
  });

  replaceManifests({
    runGh: gh.run,
    repo: REPO,
    tag: "updater",
    dir: tempDir(),
    manifests: [{ name: "latest-prerelease.json", content: "new" }],
  });

  assert.deepEqual(
    gh.calls.map(describeCall).filter((c) => c !== "list"),
    [
      "upload latest-prerelease.json.next",
      "delete 1",
      "rename 1000 -> latest-prerelease.json",
    ],
  );
  assert.deepEqual(
    [...gh.state.values()].map(({ name, content }) => ({ name, content })),
    [{ name: "latest-prerelease.json", content: "new" }],
  );
});

test("a failed upload throws without deleting any live manifest", () => {
  const gh = fakeGh({
    assets: [
      { id: 1, name: "latest-prerelease.json", content: "old-pre" },
      { id: 2, name: "latest.json", content: "old-stable" },
    ],
    failUploadOf: ["latest.json.next"],
  });

  assert.throws(
    () =>
      replaceManifests({
        runGh: gh.run,
        repo: REPO,
        tag: "updater",
        dir: tempDir(),
        manifests: [
          { name: "latest-prerelease.json", content: "new" },
          { name: "latest.json", content: "new" },
        ],
      }),
    /upload of latest.json.next failed/,
  );

  assert.equal(
    gh.calls.map(describeCall).filter((c) => c.startsWith("delete")).length,
    0,
  );
  assert.equal(gh.state.get("latest-prerelease.json").content, "old-pre");
  assert.equal(gh.state.get("latest.json").content, "old-stable");
});

test("replaces both manifests only after both temp uploads succeed", () => {
  const gh = fakeGh({
    assets: [
      { id: 1, name: "latest-prerelease.json", content: "old-pre" },
      { id: 2, name: "latest.json", content: "old-stable" },
    ],
  });

  replaceManifests({
    runGh: gh.run,
    repo: REPO,
    tag: "updater",
    dir: tempDir(),
    manifests: [
      { name: "latest-prerelease.json", content: "new-pre" },
      { name: "latest.json", content: "new-stable" },
    ],
  });

  assert.deepEqual(
    gh.calls.map(describeCall).filter((c) => c !== "list"),
    [
      "upload latest-prerelease.json.next",
      "upload latest.json.next",
      "delete 1",
      "rename 1000 -> latest-prerelease.json",
      "delete 2",
      "rename 1001 -> latest.json",
    ],
  );
  assert.equal(gh.state.get("latest-prerelease.json").content, "new-pre");
  assert.equal(gh.state.get("latest.json").content, "new-stable");
  assert.equal(gh.state.size, 2);
});

test("creates a manifest that did not exist yet", () => {
  const gh = fakeGh();

  replaceManifests({
    runGh: gh.run,
    repo: REPO,
    tag: "updater",
    dir: tempDir(),
    manifests: [{ name: "latest.json", content: "first" }],
  });

  assert.deepEqual([...gh.state.keys()], ["latest.json"]);
  assert.equal(gh.state.get("latest.json").content, "first");
});

test("removes a leftover temp asset from an earlier failed run before uploading", () => {
  const gh = fakeGh({
    assets: [
      { id: 1, name: "latest.json", content: "old" },
      { id: 7, name: "latest.json.next", content: "stale" },
    ],
  });

  replaceManifests({
    runGh: gh.run,
    repo: REPO,
    tag: "updater",
    dir: tempDir(),
    manifests: [{ name: "latest.json", content: "new" }],
  });

  const steps = gh.calls.map(describeCall).filter((c) => c !== "list");
  assert.ok(
    steps.indexOf("delete 7") < steps.indexOf("upload latest.json.next"),
    `stale temp asset should be deleted before the upload: ${steps.join(", ")}`,
  );
  assert.deepEqual([...gh.state.keys()], ["latest.json"]);
  assert.equal(gh.state.get("latest.json").content, "new");
});
