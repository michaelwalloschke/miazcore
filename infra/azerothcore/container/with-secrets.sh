#!/usr/bin/env bash
set -euo pipefail
set +x

read_secret() {
    local path="$1"
    [[ -r "$path" ]] || { echo "missing required secret: $path" >&2; exit 64; }
    local value
    value="$(<"$path")"
    [[ "$value" =~ ^[A-Za-z0-9_-]{6,64}$ ]] || {
        echo "secret $path must contain 6-64 safe ASCII characters" >&2
        exit 64
    }
    printf '%s' "$value"
}

db_password="$(read_secret /run/secrets/database_password)"
db_user="${MIAZCORE_DATABASE_USER:-acore}"
db_host="${MIAZCORE_DATABASE_HOST:-database}"

export AC_LOGIN_DATABASE_INFO="${db_host};3306;${db_user};${db_password};acore_auth"
export AC_WORLD_DATABASE_INFO="${db_host};3306;${db_user};${db_password};acore_world"
export AC_CHARACTER_DATABASE_INFO="${db_host};3306;${db_user};${db_password};acore_characters"
unset db_password

exec "$@"
