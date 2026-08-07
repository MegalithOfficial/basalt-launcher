#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "Usage: create-linux-payload.sh <deb-package> <version> <output-tar.gz>" >&2
  exit 2
fi

deb_package="$1"
version="$2"
output="$3"

if [ ! -f "$deb_package" ]; then
  echo "Debian package does not exist: $deb_package" >&2
  exit 1
fi
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Version must use x.y.z: $version" >&2
  exit 1
fi

data_archive="$(ar t "$deb_package" | sed -n '/^data\.tar\./p' | head -n 1)"
if [ -z "$data_archive" ]; then
  echo "Debian package has no data archive: $deb_package" >&2
  exit 1
fi

temporary_directory="$(mktemp -d)"
trap 'rm -rf -- "$temporary_directory"' EXIT

payload_name="Basalt.Launcher_${version}_linux_x86_64"
payload_root="$temporary_directory/$payload_name"
mkdir -p "$payload_root"
case "$data_archive" in
  *.gz)
    ar p "$deb_package" "$data_archive" | tar -xzf - -C "$payload_root"
    ;;
  *.xz)
    ar p "$deb_package" "$data_archive" | tar -xJf - -C "$payload_root"
    ;;
  *.zst)
    ar p "$deb_package" "$data_archive" | tar --zstd -xf - -C "$payload_root"
    ;;
  *)
    echo "Unsupported Debian data archive: $data_archive" >&2
    exit 1
    ;;
esac

desktop_file="$payload_root/usr/share/applications/Basalt Launcher.desktop"
if [ -f "$desktop_file" ]; then
  mv "$desktop_file" "$payload_root/usr/share/applications/basalt-launcher.desktop"
fi

install -Dm0644 /dev/null "$payload_root/usr/share/basalt-launcher/eopkg-package"
mkdir -p "$(dirname "$output")"
tar \
  --sort=name \
  --mtime='UTC 1970-01-01' \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  -czf "$output" \
  -C "$temporary_directory" \
  "$payload_name"
