#!/usr/bin/env bash
# Inject a host libwayland-client LD_PRELOAD hook into the Tauri AppImage so
# WebKitGTK does not abort with EGL_BAD_PARAMETER on Fedora and similar hosts
# whose Mesa/EGL stack mismatches the Ubuntu-bundled libwayland-client.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
appimage_dir="${repo_root}/src-tauri/target/release/bundle/appimage"
hook_name="00-wayland-compat.sh"

if [[ ! -d "$appimage_dir" ]]; then
  echo "::error::AppImage bundle directory not found: ${appimage_dir}"
  exit 1
fi

shopt -s nullglob
appdirs=("${appimage_dir}"/*.AppDir)
# Prefer Tauri's versioned AppImage name (Product_version_arch.AppImage), not
# linuxdeploy's default Product-arch.AppImage leftover from a prior run.
appimages=("${appimage_dir}"/*_*_*.AppImage)
if [[ ${#appimages[@]} -eq 0 ]]; then
  appimages=("${appimage_dir}"/*.AppImage)
fi
shopt -u nullglob

if [[ ${#appdirs[@]} -eq 0 ]]; then
  echo "::error::No *.AppDir found under ${appimage_dir}"
  exit 1
fi

if [[ ${#appimages[@]} -eq 0 ]]; then
  echo "::error::No *.AppImage found under ${appimage_dir}"
  exit 1
fi

if [[ ${#appdirs[@]} -ne 1 ]]; then
  echo "::warning::Expected one AppDir, found ${#appdirs[@]} — using the first"
fi
if [[ ${#appimages[@]} -ne 1 ]]; then
  echo "::warning::Expected one versioned AppImage, found ${#appimages[@]} — using the first"
fi

appdir="${appdirs[0]}"
appimage="${appimages[0]}"
hooks_dir="${appdir}/apprun-hooks"

mkdir -p "$hooks_dir"
cat >"${hooks_dir}/${hook_name}" <<'EOF'
# Force host libwayland-client to avoid EGL_BAD_PARAMETER on Fedora/others
if [ -z "${LD_PRELOAD:-}" ]; then
  for lib in \
    /usr/lib64/libwayland-client.so \
    /usr/lib/x86_64-linux-gnu/libwayland-client.so \
    /usr/lib/libwayland-client.so \
    /usr/lib/aarch64-linux-gnu/libwayland-client.so \
    /usr/lib/arm-linux-gnueabihf/libwayland-client.so; do
    if [ -f "$lib" ]; then
      export LD_PRELOAD="$lib"
      break
    fi
  done
fi
export WEBKIT_DISABLE_DMABUF_RENDERER=1
EOF
chmod +x "${hooks_dir}/${hook_name}"
echo "Installed AppRun hook: ${hooks_dir}/${hook_name}"

find_tool() {
  local name="$1"
  local candidate
  for candidate in \
    "${HOME}/.cache/tauri/${name}" \
    "${XDG_CACHE_HOME:-}/tauri/${name}"; do
    if [[ -n "$candidate" && -f "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

plugin=""
linuxdeploy=""
if plugin="$(find_tool linuxdeploy-plugin-appimage-x86_64.AppImage)" \
  || plugin="$(find_tool linuxdeploy-plugin-appimage-aarch64.AppImage)"; then
  chmod +x "$plugin"
  echo "Repacking $(basename "$appimage") with ${plugin}"
  (
    cd "$appimage_dir"
    # Pack only — do not re-run linuxdeploy dependency deployment.
    # LDAI_OUTPUT must overwrite Tauri's versioned AppImage, not create
    # Modkist-x86_64.AppImage in an unrelated cwd.
    LDAI_OUTPUT="$(basename "$appimage")" \
      "$plugin" --appimage-extract-and-run --appdir "$(basename "$appdir")"
  )
elif linuxdeploy="$(find_tool linuxdeploy-x86_64.AppImage)" \
  || linuxdeploy="$(find_tool linuxdeploy-aarch64.AppImage)"; then
  chmod +x "$linuxdeploy"
  echo "Repacking $(basename "$appimage") with ${linuxdeploy}"
  (
    cd "$appimage_dir"
    LDAI_OUTPUT="$(basename "$appimage")" \
      "$linuxdeploy" --appimage-extract-and-run \
      --appdir "$(basename "$appdir")" \
      --output appimage
  )
else
  echo "::error::Neither linuxdeploy-plugin-appimage nor linuxdeploy found in ~/.cache/tauri"
  exit 1
fi

if [[ ! -f "${hooks_dir}/${hook_name}" ]]; then
  echo "::error::Hook was removed from AppDir during repack: ${hooks_dir}/${hook_name}"
  exit 1
fi

extract_dir="$(mktemp -d)"
cleanup() { rm -rf "$extract_dir"; }
trap cleanup EXIT
(
  cd "$extract_dir"
  "$appimage" --appimage-extract >/dev/null
)
if [[ ! -f "${extract_dir}/squashfs-root/apprun-hooks/${hook_name}" ]]; then
  echo "::error::Hook missing from repacked AppImage: ${appimage}"
  exit 1
fi
echo "Verified hook in ${appimage}"

# Remove linuxdeploy's default-named leftover if present (e.g. Modkist-x86_64.AppImage).
shopt -s nullglob
for leftover in "${appimage_dir}"/*-x86_64.AppImage "${appimage_dir}"/*-aarch64.AppImage; do
  if [[ "$(basename "$leftover")" != "$(basename "$appimage")" ]]; then
    echo "Removing leftover AppImage: ${leftover}"
    rm -f "$leftover"
  fi
done
shopt -u nullglob

echo "AppImage Wayland compatibility patch complete"
