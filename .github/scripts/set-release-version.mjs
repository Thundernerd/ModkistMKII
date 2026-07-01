import { readCurrentVersion, updateVersionFiles } from "./version-files.mjs";

const printOnly = process.argv.includes("--print-only");
const version = process.env.VERSION;

if (!version) {
  console.error("::error::VERSION is not set");
  process.exit(1);
}

const versionPattern = /^\d+\.\d+\.\d+(-(alpha|beta|rc)\.\d+)?$/;

if (!versionPattern.test(version)) {
  console.error(
    `::error::Expected major.minor.patch or major.minor.patch-(alpha|beta|rc).N, got ${version}`,
  );
  process.exit(1);
}

if (!printOnly) {
  const previous = updateVersionFiles(version);
  console.error(`Set release version: ${previous} -> ${version}`);
}

process.stdout.write(version);
