// Prints the Tauri config override that gives the rpm its version and release.
import { toRpmVersion } from "./rpm-version.mjs";

const appVersion = process.env.VERSION;

if (!appVersion) {
  console.error("::error::VERSION is not set");
  process.exit(1);
}

try {
  const { version, release } = toRpmVersion(appVersion);
  process.stdout.write(
    JSON.stringify({ version, bundle: { linux: { rpm: { release } } } }),
  );
} catch (error) {
  console.error(`::error::${error instanceof Error ? error.message : error}`);
  process.exit(1);
}
