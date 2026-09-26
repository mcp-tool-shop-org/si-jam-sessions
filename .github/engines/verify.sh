#!/usr/bin/env bash
# Checks an installed JavaScript engine against its committed manifest, before
# the golden job lets it run anything.
#
#   verify.sh <engine path> <manifest>   exit 0 when the installed engine is
#                                        exactly the manifest; exit 1, with the
#                                        difference printed, when it is not
#   verify.sh --print <engine path>      print the installed engine's manifest
#
# A manifest has one line per entry under the engine's directory, sorted by
# path in byte order:
#   file <sha256> <path>       a regular file and its SHA-256
#   link <path> -> <target>    a symbolic link and its target, as readlink
#                              prints it (links are listed, never followed)
#   dir <path>                 a directory
# For an engine that is one file, the manifest is that file's `file` line,
# under its base name. Any other kind of entry is listed as `other <path>`,
# which no manifest holds. Lines starting with `#` are comments.
#
# So the check fails when any file changes, and when a file, a link or a
# directory is added, removed or pointed elsewhere: a library the loader could
# pick up is pinned as surely as the shell itself.
set -euo pipefail

manifest() {
  local root="$1"
  if [ -f "$root" ] && [ ! -L "$root" ]; then
    printf 'file %s %s\n' "$(sha256sum < "$root" | cut -c1-64)" "$(basename "$root")"
    return
  fi
  cd "$root"
  find . -mindepth 1 -print0 | LC_ALL=C sort -z | while IFS= read -r -d '' entry; do
    path="${entry#./}"
    if [ -L "$entry" ]; then
      printf 'link %s -> %s\n' "$path" "$(readlink "$entry")"
    elif [ -f "$entry" ]; then
      printf 'file %s %s\n' "$(sha256sum < "$entry" | cut -c1-64)" "$path"
    elif [ -d "$entry" ]; then
      printf 'dir %s\n' "$path"
    else
      printf 'other %s\n' "$path"
    fi
  done
}

if [ "${1:-}" = "--print" ] && [ "$#" -eq 2 ]; then
  manifest "$2"
  exit 0
fi
if [ "$#" -ne 2 ]; then
  echo "usage: verify.sh <engine path> <manifest> | verify.sh --print <engine path>" >&2
  exit 2
fi

engine="$1"
pins="$2"
if [ ! -e "$engine" ]; then
  echo "verify: $engine is not installed" >&2
  exit 1
fi
if [ ! -f "$pins" ]; then
  echo "verify: there is no manifest at $pins" >&2
  exit 1
fi
installed="$(manifest "$engine")"
if diff -u <(grep -v '^#' "$pins") <(printf '%s\n' "$installed"); then
  echo "verify: $engine is exactly $pins"
else
  echo "verify: $engine is not what $pins pins; it must not run" >&2
  exit 1
fi
