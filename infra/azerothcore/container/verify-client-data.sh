#!/usr/bin/env bash
set -euo pipefail

readonly data_dir=/azerothcore/env/dist/data
readonly marker="$data_dir/.miazcore-client-data.lock"
readonly release=v20.0
readonly asset=Data.zip
readonly url=https://github.com/wowgaming/client-data/releases/download/v20.0/Data.zip
readonly expected_size=1196168257
readonly expected_sha=a3d4df635ae6c2c8f08052c32a79e0f806955150ad36b014a823dd08a32a4610

validate_installation() {
    [[ -f "$marker" ]] || return 1
    grep -qx "release=$release" "$marker" || return 1
    grep -qx "asset=$asset" "$marker" || return 1
    grep -qx "size=$expected_size" "$marker" || return 1
    grep -qx "sha256=$expected_sha" "$marker" || return 1
    local required
    for required in dbc maps vmaps mmaps; do
        [[ -d "$data_dir/$required" ]] || return 1
    done
}

if validate_installation; then
    echo "client-data: verified cache hit ($release, $expected_sha)"
    exit 0
fi

if [[ -e "$marker" ]]; then
    echo "client-data: volume is populated but does not match the artifact lock; run reset-all" >&2
    exit 65
fi

# Docker copies the image's declared empty data-directory skeleton into a new
# named volume. Accept only that exact, empty skeleton; anything else is drift.
for entry in "$data_dir"/*; do
    [[ -e "$entry" ]] || continue
    case "$(basename "$entry")" in
        Cameras|dbc|maps|vmaps|mmaps)
            [[ -d "$entry" ]] || { echo "client-data: unexpected non-directory $entry" >&2; exit 65; }
            find "$entry" -mindepth 1 -print -quit | grep -q . && {
                echo "client-data: unmarked data exists in $entry; run reset-all" >&2
                exit 65
            }
            ;;
        *)
            echo "client-data: unexpected unmarked entry $entry; run reset-all" >&2
            exit 65
            ;;
    esac
done
rmdir "$data_dir"/{Cameras,dbc,maps,vmaps,mmaps} 2>/dev/null || true

archive="$(mktemp /tmp/miazcore-data.XXXXXX.zip)"
stage="$data_dir/.staging-$$"
cleanup() { rm -f "$archive"; rm -rf "$stage"; }
trap cleanup EXIT
mkdir -p "$stage"

echo "client-data: downloading $release/$asset"
curl --fail --location --retry 4 --retry-all-errors --output "$archive" "$url"
actual_size="$(stat --format=%s "$archive")"
[[ "$actual_size" == "$expected_size" ]] || {
    echo "client-data: size mismatch: expected $expected_size, got $actual_size" >&2
    exit 65
}
printf '%s  %s\n' "$expected_sha" "$archive" | sha256sum --check --status || {
    echo "client-data: SHA-256 mismatch" >&2
    exit 65
}

echo "client-data: extracting verified archive"
unzip -q "$archive" -d "$stage"
source_dir="$stage"
[[ -d "$stage/Data" ]] && source_dir="$stage/Data"
for required in dbc maps vmaps mmaps; do
    [[ -d "$source_dir/$required" ]] || {
        echo "client-data: archive is missing $required/" >&2
        exit 65
    }
done

find "$source_dir" -mindepth 1 -maxdepth 1 -exec mv {} "$data_dir/" \;
cat >"$marker" <<EOF
release=$release
asset=$asset
size=$expected_size
sha256=$expected_sha
EOF
validate_installation || { echo "client-data: post-extraction validation failed" >&2; exit 65; }
echo "client-data: installed and verified ($release, $expected_sha)"
