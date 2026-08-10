#!/usr/bin/env bash
# Inject a host libwayland-client LD_PRELOAD hook into the Tauri AppImage so
# WebKitGTK does not abort with EGL_BAD_PARAMETER on Fedora and similar hosts
# whose Mesa/EGL stack mismatches the Ubuntu-bundled libwayland-client.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
appimage_dir="${repo_root}/src-tauri/target/release/bundle/appimage"

if [[ ! -d "$appimage_dir" ]]; then
  echo "::error::AppImage bundle directory not found: ${appimage_dir}"
  exit 1
fi

shopt -s nullglob
appdirs=("${appimage_dir}"/*.AppDir)
appimages=("${appimage_dir}"/*.AppImage)
shopt -u nullglob

if [[ ${#appdirs[@]} -eq 0 ]]; then
  echo "::error::No *.AppDir found under ${appimage_dir}"
  exit 1
fi

if [[ ${#appimages[@]} -eq 0 ]]; then
  echo "::error::No *.AppImage found under ${appimage_dir}"
  exit 1
fi

if [[ ${#appdirs[@]} -gt 1 ]]; then
  echo "::warning::Multiple AppDirs found; patching all of them"
fi

hook_name="00-wayland-compat.sh"

for appdir in "${appdirs[@]}"; do
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
done

linuxdeploy=""
for candidate in \
  "${HOME}/.cache/tauri/linuxdeploy-x86_64.AppImage" \
  "${HOME}/.cache/tauri/linuxdeploy-aarch64.AppImage" \
  "${XDG_CACHE_HOME:-}/tauri/linuxdeploy-x86_64.AppImage" \
  "${XDG_CACHE_HOME:-}/tauri/linuxdeploy-aarch64.AppImage"; do
  if [[ -n "$candidate" && -f "$candidate" ]]; then
    linuxdeploy="$candidate"
    break
  fi
done

if [[ -z "$linuxdeploy" ]]; then
  echo "::error::linuxdeploy AppImage not found in ~/.cache/tauri (run tauri build first)"
  exit 1
fi

chmod +x "$linuxdeploy"
echo "Repacking AppImage(s) with ${linuxdeploy}"

for appdir in "${appdirs[@]}"; do
  "$linuxdeploy" --appimage-extract-and-run --appdir "$appdir" --output appimage
done

verify_failed=0
for appimage in "${appimages[@]}"; do
  extract_dir="$(mktemp -d)"
  (
    cd "$extract_dir"
    "$appimage" --appimage-extract >/dev/null
  )
  if [[ ! -f "${extract_dir}/squashfs-root/apprun-hooks/${hook_name}" ]]; then
    echo "::error::Hook missing from repacked AppImage: ${appimage}"
    verify_failed=1
  else
    echo "Verified hook in ${appimage}"
  fi
  rm -rf "$extract_dir"
done

if [[ "$verify_failed" -ne 0 ]]; then
  exit 1
fi

echo "AppImage Wayland compatibility patch complete"
