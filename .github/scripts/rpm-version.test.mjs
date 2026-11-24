import assert from "node:assert/strict";
import test from "node:test";
import { toRpmVersion } from "./rpm-version.mjs";

test("keeps a stable version, with release 1", () => {
  assert.deepEqual(toRpmVersion("1.2.3"), { version: "1.2.3", release: "1" });
});

for (const channel of ["alpha", "beta", "rc"]) {
  test(`moves the ${channel} part to a release below the stable one`, () => {
    assert.deepEqual(toRpmVersion(`1.2.3-${channel}.10`), {
      version: "1.2.3",
      release: `0.${channel}.10`,
    });
  });
}

test("moves the Build workflow's run number to a release below alpha", () => {
  assert.deepEqual(toRpmVersion("1.2.3-37"), {
    version: "1.2.3",
    release: "0~37",
  });
});

for (const version of ["", "1.2", "v1.2.3", "1.2.3-", "1.2.3-rc", "1.2.3-37.1"]) {
  test(`rejects ${JSON.stringify(version)}`, () => {
    assert.throws(() => toRpmVersion(version), /Expected major\.minor\.patch/);
  });
}
