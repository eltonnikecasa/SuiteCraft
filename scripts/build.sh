#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

for command_name in node npm cargo rustc pkg-config curl; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Erro: dependência obrigatória ausente: $command_name" >&2
    exit 1
  fi
done

for package_name in webkit2gtk-4.1 javascriptcoregtk-4.1 libsoup-3.0; do
  if ! pkg-config --exists "$package_name"; then
    echo "Erro: biblioteca de desenvolvimento ausente: $package_name" >&2
    exit 1
  fi
done

export CARGO_BUILD_JOBS=15
runtime_file="$repo_root/src-tauri/target/appimage-runtime-x86_64"
if [[ ! -s "$runtime_file" ]]; then
  mkdir -p "$(dirname "$runtime_file")"
  runtime_temp="${runtime_file}.tmp"
  curl --fail --location --retry 3 --connect-timeout 15 \
    --output "$runtime_temp" \
    https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64
  if [[ "$(head -c 4 "$runtime_temp")" != $'\x7fELF' ]]; then
    rm -f "$runtime_temp"
    echo "Erro: o runtime AppImage baixado não é um ELF válido" >&2
    exit 1
  fi
  mv "$runtime_temp" "$runtime_file"
fi
export LDAI_RUNTIME_FILE="$runtime_file"
npm run tauri build -- --bundles appimage

artifact_dir="$repo_root/src-tauri/target/release/bundle/appimage"
artifact="$(find "$artifact_dir" -maxdepth 1 -type f -name '*.AppImage' -print -quit 2>/dev/null || true)"
if [[ -z "$artifact" ]]; then
  echo "Erro: o build terminou sem gerar um AppImage em $artifact_dir" >&2
  exit 1
fi

echo "AppImage gerado: $artifact"
