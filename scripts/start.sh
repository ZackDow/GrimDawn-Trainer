#!/bin/sh
# Portable launcher: run with `sh start.sh`. Works after copies that drop the
# executable bit (FAT/exFAT/NTFS sticks, cloud drives, zip tools) or onto
# noexec mounts by staging the app in the user's cache directory.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
app="$here/grim-dawn-trainer"
[ -f "$app" ] || { echo "grim-dawn-trainer not found beside start.sh" >&2; exit 1; }

[ -x "$app" ] || chmod +x "$app" 2>/dev/null || true
noexec=false
if command -v findmnt >/dev/null 2>&1; then
    case ",$(findmnt -no OPTIONS --target "$here" 2>/dev/null)," in
        *,noexec,*) noexec=true ;;
    esac
fi
if [ -x "$app" ] && [ "$noexec" = false ]; then
    exec "$app" "$@"
fi

stage="${XDG_CACHE_HOME:-$HOME/.cache}/grim-dawn-trainer-portable"
rm -rf "$stage"
mkdir -p "$stage"
cp "$app" "$stage/grim-dawn-trainer"
cp -R "$here/runtime" "$stage/runtime"
chmod +x "$stage/grim-dawn-trainer"
exec "$stage/grim-dawn-trainer" "$@"
