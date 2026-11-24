const VERSION_PATTERN = /^(\d+\.\d+\.\d+)(?:-((?:alpha|beta|rc)\.\d+))?$/;

/**
 * Split a semver app version into an rpm version and release.
 *
 * rpm sorts 1.2.3-rc.1 after 1.2.3, so `rpm -U` refuses to update an rc to the
 * stable release. A prerelease is put in the release field instead, below the
 * stable release's 1: 1.2.3-rc.1 becomes version 1.2.3, release 0.rc.1.
 */
export function toRpmVersion(version) {
  const match = VERSION_PATTERN.exec(version);
  if (!match) {
    throw new Error(
      `Expected major.minor.patch or major.minor.patch-(alpha|beta|rc).N, got ${version}`,
    );
  }

  const [, base, prerelease] = match;
  return { version: base, release: prerelease ? `0.${prerelease}` : "1" };
}
