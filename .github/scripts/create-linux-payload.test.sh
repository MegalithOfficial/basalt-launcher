#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
temporary_directory="$(mktemp -d)"
trap 'rm -rf -- "$temporary_directory"' EXIT

fixture_root="$temporary_directory/fixture"
mkdir -p "$fixture_root/usr/bin" "$fixture_root/usr/share/applications"
printf '#!/usr/bin/env bash\n' > "$fixture_root/usr/bin/basalt-launcher"
chmod 0755 "$fixture_root/usr/bin/basalt-launcher"
printf '[Desktop Entry]\nName=Basalt Launcher\n' > \
  "$fixture_root/usr/share/applications/Basalt Launcher.desktop"

printf '2.0\n' > "$temporary_directory/debian-binary"
tar -czf "$temporary_directory/control.tar.gz" --files-from /dev/null
tar -czf "$temporary_directory/data.tar.gz" -C "$fixture_root" usr
(
  cd "$temporary_directory"
  ar r fixture.deb debian-binary control.tar.gz data.tar.gz >/dev/null
)

first_payload="$temporary_directory/first.tar.gz"
second_payload="$temporary_directory/second.tar.gz"
"$repository_root/.github/scripts/create-linux-payload.sh" \
  "$temporary_directory/fixture.deb" 1.2.3 "$first_payload"
"$repository_root/.github/scripts/create-linux-payload.sh" \
  "$temporary_directory/fixture.deb" 1.2.3 "$second_payload"

cmp "$first_payload" "$second_payload"
tar -tzf "$first_payload" | grep -Fx \
  'Basalt.Launcher_1.2.3_linux_x86_64/usr/share/applications/basalt-launcher.desktop'
tar -tzf "$first_payload" | grep -Fx \
  'Basalt.Launcher_1.2.3_linux_x86_64/usr/share/basalt-launcher/eopkg-package'
if tar -tzf "$first_payload" | grep -Fq 'Basalt Launcher.desktop'; then
  echo "Payload still contains the unnormalized desktop file name." >&2
  exit 1
fi
