import fs from "node:fs";
import { toMsiVersion } from "./msi-version.mjs";

const printOnly = process.argv.includes("--print-only");
const version = process.env.VERSION;

if (!version) {
  console.error("::error::VERSION is not set");
  process.exit(1);
}

let msiVersion;
try {
  msiVersion = toMsiVersion(version);
} catch (error) {
  console.error(`::error::${error instanceof Error ? error.message : error}`);
  process.exit(1);
}

if (!printOnly) {
  const configPath = "src-tauri/tauri.conf.json";
  const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
  config.bundle ??= {};
  config.bundle.windows ??= {};
  config.bundle.windows.wix ??= {};
  config.bundle.windows.wix.version = msiVersion;
  fs.writeFileSync(configPath, JSON.stringify(config, null, 2) + "\n");
  console.error(`Set MSI version: ${version} -> ${msiVersion}`);
}

process.stdout.write(msiVersion);
