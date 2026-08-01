#!/usr/bin/env bash
set -euo pipefail
set +x

read_secret() {
    local path="$1" value
    [[ -r "$path" ]] || { echo "fixture: missing secret $path" >&2; exit 64; }
    value="$(<"$path")"
    [[ "$value" =~ ^[A-Za-z0-9_-]{6,64}$ ]] || { echo "fixture: invalid secret format in $path" >&2; exit 64; }
    printf '%s' "$value"
}

db_password="$(read_secret /run/secrets/database_password)"
account="$(read_secret /run/secrets/fixture_account)"
account_password="$(read_secret /run/secrets/fixture_password)"
pair_a_account="$(read_secret /run/secrets/fixture_pair_a_account)"
pair_a_password="$(read_secret /run/secrets/fixture_pair_a_password)"
pair_b_account="$(read_secret /run/secrets/fixture_pair_b_account)"
pair_b_password="$(read_secret /run/secrets/fixture_pair_b_password)"
for password in "$account_password" "$pair_a_password" "$pair_b_password"; do
    (( ${#password} <= 16 )) || { echo "fixture: account password exceeds the build-12340 16-character limit" >&2; exit 64; }
done
mysql_defaults=/run/miazcore/mysql.cnf
mkdir -p /run/miazcore
chmod 700 /run/miazcore
cat >"$mysql_defaults" <<EOF
[client]
user=acore
password=$db_password
host=database
port=3306
protocol=tcp
EOF
chmod 600 "$mysql_defaults"
unset db_password

mysql_auth() { mysql --defaults-extra-file="$mysql_defaults" --batch --skip-column-names acore_auth "$@"; }
mysql_chars() { mysql --defaults-extra-file="$mysql_defaults" --batch --skip-column-names acore_characters "$@"; }

realm_address="${MIAZCORE_REALM_ADDRESS:-127.0.0.1}"
[[ "$realm_address" =~ ^[A-Za-z0-9._:-]+$ ]] || { echo "fixture: invalid realm address" >&2; exit 64; }
mysql_auth <<SQL
INSERT INTO realmlist
    (id, name, address, localAddress, localSubnetMask, port, icon, flag, timezone, allowedSecurityLevel, population, gamebuild)
VALUES
    (1, 'Miazcore Reference Realm', '$realm_address', '127.0.0.1', '255.255.255.0', 8085, 0, 0, 1, 0, 0, 12340)
ON DUPLICATE KEY UPDATE
    name=VALUES(name), address=VALUES(address), localAddress=VALUES(localAddress),
    localSubnetMask=VALUES(localSubnetMask), port=VALUES(port), icon=VALUES(icon),
    flag=VALUES(flag), timezone=VALUES(timezone), allowedSecurityLevel=VALUES(allowedSecurityLevel),
    population=VALUES(population), gamebuild=VALUES(gamebuild);
SQL

mode="${1:-provision}"
fixtures=(
    /miazcore/fixtures/reference-character.pdump
    /miazcore/fixtures/reference-pair-a-character.pdump
    /miazcore/fixtures/reference-pair-b-character.pdump
)
accounts=("$account" "$pair_a_account" "$pair_b_account")
passwords=("$account_password" "$pair_a_password" "$pair_b_password")
characters=(Miaztest Miazpaira Miazpairb)
command_file=/run/miazcore/worldserver.commands
sanitize_output() {
    local line
    while IFS= read -r line || [[ -n "$line" ]]; do
        for password in "${passwords[@]}"; do
            line="${line//$password/[REDACTED]}"
        done
        printf '%s\n' "$line"
    done
}

run_worldserver() {
    chmod 600 "$command_file"
    set +e
    /azerothcore/env/dist/bin/worldserver <"$command_file" 2>&1 | sanitize_output
    local worldserver_status="${PIPESTATUS[0]}"
    set -e
    rm -f "$command_file"
    [[ "$worldserver_status" == 0 ]] || { echo "fixture: provisioning worldserver exited $worldserver_status" >&2; exit "$worldserver_status"; }
}

if [[ "$mode" == export ]]; then
    case "${MIAZCORE_EXPORT_PROFILE:-reference}" in
        reference) fixture=reference-character.pdump; export_character=Miaztest ;;
        pair-a) fixture=reference-pair-a-character.pdump; export_character=Miazpaira ;;
        pair-b) fixture=reference-pair-b-character.pdump; export_character=Miazpairb ;;
        *) echo "fixture: unsupported export profile" >&2; exit 64 ;;
    esac
    [[ "$fixture" != */* && "$fixture" != *\\* ]] || { echo "fixture: PDump.NoPaths requires an export basename" >&2; exit 64; }
    printf 'pdump write %s %s\nserver shutdown 1\n' "$fixture" "$export_character" >"$command_file"
    run_worldserver
    fixture_size="$(stat --format=%s "$fixture" 2>/dev/null || printf 0)"
    (( fixture_size > 1024 )) || { echo "fixture: pdump export was not created or is implausibly small" >&2; exit 65; }
    unset account_password
    echo "fixture: server-generated dump exported"
    exit 0
fi

if [[ "$mode" == bootstrap-pair-accounts ]]; then
    : >"$command_file"
    for index in 1 2; do
        account_id="$(mysql_auth --execute="SELECT id FROM account WHERE username=UPPER('${accounts[index]}');")"
        if [[ -z "$account_id" ]]; then
            printf 'account create %s %s\n' "${accounts[index]}" "${passwords[index]}" >>"$command_file"
        fi
    done
    printf 'server shutdown 1\n' >>"$command_file"
    run_worldserver
    echo "fixture: Pair accounts prepared through Worldserver"
    exit 0
fi

[[ "$mode" == provision ]] || { echo "fixture: unsupported mode $mode" >&2; exit 64; }
for fixture in "${fixtures[@]}"; do
    [[ -f "$fixture" ]] || { echo "fixture: required reviewed pdump is missing" >&2; exit 66; }
done

# A newly created account is not added to the already-running worldserver's
# account cache. Create it in its own short-lived run, then restart before any
# command that resolves the account by name.
account_ids=()
: >"$command_file"
for index in "${!accounts[@]}"; do
    account_id="$(mysql_auth --execute="SELECT id FROM account WHERE username=UPPER('${accounts[index]}');")"
    if [[ -z "$account_id" ]]; then
        printf 'account create %s %s\n' "${accounts[index]}" "${passwords[index]}" >>"$command_file"
    fi
    account_ids[index]="$account_id"
done
if [[ -s "$command_file" ]]; then
    printf 'server shutdown 1\n' >>"$command_file"
    run_worldserver
fi
for index in "${!accounts[@]}"; do
    account_ids[index]="$(mysql_auth --execute="SELECT id FROM account WHERE username=UPPER('${accounts[index]}');")"
    [[ "${account_ids[index]}" =~ ^[0-9]+$ ]] || { echo "fixture: account was not provisioned" >&2; exit 65; }
    mysql_auth --execute="UPDATE account SET expansion=2, locked=0, lock_country='00', totp_secret=NULL WHERE id=${account_ids[index]};"
    character_count="$(mysql_chars --execute="SELECT COUNT(*) FROM characters WHERE name='${characters[index]}';")"
    [[ "$character_count" == 0 || "$character_count" == 1 ]] || { echo "fixture: duplicate fixture character" >&2; exit 65; }
    printf 'account set password %s %s %s\naccount set addon %s 2\n' \
        "${accounts[index]}" "${passwords[index]}" "${passwords[index]}" "${accounts[index]}" >>"$command_file"
    if [[ "$character_count" == 0 ]]; then
        printf 'pdump load %s %s %s\n' "${fixtures[index]}" "${accounts[index]}" "${characters[index]}" >>"$command_file"
    fi
done
printf 'server shutdown 1\n' >>"$command_file"
run_worldserver
unset account_password pair_a_password pair_b_password

for index in "${!accounts[@]}"; do
    owner="$(mysql_chars --execute="SELECT account FROM characters WHERE name='${characters[index]}';")"
    [[ "$owner" == "${account_ids[index]}" ]] || { echo "fixture: character ownership invariant failed" >&2; exit 65; }
done
echo "fixture: three-fixture realm and account invariant verified"
