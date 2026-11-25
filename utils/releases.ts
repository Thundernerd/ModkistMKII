const RELEASES_URL = "https://github.com/Thundernerd/ModkistMKII/releases";

/** The GitHub page to download a Modkist version from, or the latest release. */
export function releasePageUrl(version?: string | null): string {
  return version
    ? `${RELEASES_URL}/tag/v${version}`
    : `${RELEASES_URL}/latest`;
}
