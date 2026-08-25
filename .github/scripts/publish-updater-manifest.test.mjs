import assert from "node:assert/strict";
import test from "node:test";
import {
  assetUrl,
  buildManifest,
  buildPlatforms,
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
