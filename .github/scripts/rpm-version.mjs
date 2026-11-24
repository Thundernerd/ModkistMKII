const VERSION_PATTERN = /^(\d+\.\d+\.\d+)(?:-((?:alpha|beta|rc)\.\d+|\d+))?$/;

/**
 * Split a semver app version into an rpm version and release.
 *
 * rpm sorts 1.2.3-rc.1 after 1.2.3, so `rpm -U` refuses to update an rc to the
 * stable release. A prerelease is put in the release field instead, below the
 * stable release's 1: 1.2.3-rc.1 becomes version 1.2.3, release 0.rc.1.
 *
 * The Build workflow's 1.2.3-37 becomes release 0~37. semver sorts it before
 * 1.2.3-alpha.1, but rpm sorts 0.37 after 0.alpha.1; `~` sorts before both.
 */
export function toRpmVersion(version) {
  const match = VERSION_PATTERN.exec(version);
  if (!match) {
    throw new Error(
      `Expected major.minor.patch or major.minor.patch-(alpha|beta|rc).N or major.minor.patch-N, got ${version}`,
    );
  }

  const [, base, prerelease] = match;
  if (!prerelease) {
    return { version: base, release: "1" };
  }
  const separator = /^\d/.test(prerelease) ? "~" : ".";
  return { version: base, release: `0${separator}${prerelease}` };
}
