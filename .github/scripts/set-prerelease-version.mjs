import { readCurrentVersion, updateVersionFiles } from "./version-files.mjs";

const printOnly = process.argv.includes("--print-only");
const runNumber = process.env.RUN_NUMBER;

if (!runNumber) {
  console.error("::error::RUN_NUMBER is not set");
  process.exit(1);
}

const current = readCurrentVersion();
const match = current.match(/^(\d+\.\d+\.\d+)(?:-.+)?$/);

if (!match) {
  console.error(
    `::error::Expected major.minor.patch or major.minor.patch-suffix, got ${current}`,
  );
  process.exit(1);
}

const newVersion = `${match[1]}-${runNumber}`;

if (!printOnly) {
  const previous = updateVersionFiles(newVersion);
  console.error(`Set build version: ${previous} -> ${newVersion}`);
}

process.stdout.write(newVersion);
