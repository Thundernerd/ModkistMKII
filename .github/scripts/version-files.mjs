import fs from "node:fs";

export function readCurrentVersion() {
  return JSON.parse(fs.readFileSync("package.json", "utf8")).version;
}

export function updateVersionFiles(newVersion) {
  const current = readCurrentVersion();

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

  return current;
}
