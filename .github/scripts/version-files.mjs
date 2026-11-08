import fs from "node:fs";

// Every file the release bump rewrites; release.yml must stage all of them.
export const VERSION_FILES = [
  "package.json",
  "package-lock.json",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
];

export function readCurrentVersion() {
  return JSON.parse(fs.readFileSync("package.json", "utf8")).version;
}

function updateJson(path, update) {
  const json = JSON.parse(fs.readFileSync(path, "utf8"));
  update(json);
  fs.writeFileSync(path, JSON.stringify(json, null, 2) + "\n");
}

export function updateVersionFiles(newVersion) {
  const current = readCurrentVersion();

  updateJson("package.json", (pkg) => {
    pkg.version = newVersion;
  });

  updateJson("package-lock.json", (npmLock) => {
    npmLock.version = newVersion;
    npmLock.packages[""].version = newVersion;
  });

  updateJson("src-tauri/tauri.conf.json", (tauri) => {
    tauri.version = newVersion;
  });

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
