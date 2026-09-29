#!/bin/sh
# The process starts as root only long enough to drop privileges. The host
# then runs as uid 10001, or as HOST_UID:HOST_GID when the caller sets them,
# so a file written into a mounted directory keeps the caller's owner.
set -eu

if [ "$(id -u)" = 0 ]; then
    uid="${HOST_UID:-10001}"
    gid="${HOST_GID:-10001}"
    case "$uid$gid" in
        *[!0-9]*)
            echo "host: HOST_UID and HOST_GID must be numbers" >&2
            exit 1
            ;;
    esac
    if [ "$uid" != 10001 ] || [ "$gid" != 10001 ]; then
        usermod -o -u "$uid" jam
        groupmod -o -g "$gid" jam
    fi
    export HOME="${HOME:-/home/jam}"
    exec setpriv --reuid "$uid" --regid "$gid" --init-groups --inh-caps=-all -- /usr/local/bin/host "$@"
fi

exec /usr/local/bin/host "$@"
