import fs from "node:fs";

const printOnly = process.argv.includes("--print-only");
const runNumber = process.env.RUN_NUMBER;

if (!runNumber) {
  console.error("::error::RUN_NUMBER is not set");
  process.exit(1);
}

const current = JSON.parse(fs.readFileSync("package.json", "utf8")).version;
const match = current.match(/^(\d+\.\d+\.\d+)(?:-.+)?$/);

if (!match) {
  console.error(
    `::error::Expected major.minor.patch or major.minor.patch-suffix, got ${current}`,
  );
  process.exit(1);
}

const newVersion = `${match[1]}-${runNumber}`;

if (!printOnly) {
  const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
  pkg.version = newVersion;
  fs.writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\n");

  const tauri = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
  tauri.version = newVersion;
  fs.writeFileSync(
    "src-tauri/tauri.conf.json",
    JSON.stringify(tauri, null, 2) + "\n",
  );

  const cargoTomlPath = "src-tauri/Cargo.toml";
  const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
  fs.writeFileSync(
    cargoTomlPath,
    cargoToml.replace(/^version = ".*"$/m, `version = "${newVersion}"`),
  );

  const cargoLockPath = "src-tauri/Cargo.lock";
  const cargoLock = fs.readFileSync(cargoLockPath, "utf8").split("\n");
  let seenPackage = false;
  const updatedLock = cargoLock
    .map((line) => {
      if (line === 'name = "modkistmkii"') {
        seenPackage = true;
        return line;
      }
      if (seenPackage && line.startsWith('version = "')) {
        seenPackage = false;
        return `version = "${newVersion}"`;
      }
      return line;
    })
    .join("\n");
  fs.writeFileSync(cargoLockPath, updatedLock);

  console.error(`Set build version: ${current} -> ${newVersion}`);
}

process.stdout.write(newVersion);
