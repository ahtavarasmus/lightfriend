#!/usr/bin/env bash
set -euo pipefail

readonly INDEXNOW_KEY="3d870fd8-5621-466c-8d7b-e326e09db85e"
readonly INDEXNOW_ENDPOINT="${INDEXNOW_ENDPOINT:-https://api.indexnow.org/indexnow}"
readonly SITE_ORIGIN="https://lightfriend.ai"

if [[ $# -eq 0 ]]; then
    echo "Usage: $0 <lightfriend.ai URL> [URL ...]" >&2
    echo "Submit only URLs whose content was added, changed, redirected, or deleted." >&2
    exit 2
fi

for url in "$@"; do
    case "$url" in
        "$SITE_ORIGIN"|"$SITE_ORIGIN"/*) ;;
        *)
            echo "Refusing URL outside $SITE_ORIGIN: $url" >&2
            exit 2
            ;;
    esac

    response_file="$(mktemp)"
    trap 'rm -f "$response_file"' EXIT

    status="$(curl --silent --show-error \
        --output "$response_file" \
        --write-out '%{http_code}' \
        --get \
        --data-urlencode "url=$url" \
        --data-urlencode "key=$INDEXNOW_KEY" \
        "$INDEXNOW_ENDPOINT")"

    case "$status" in
        200|202)
            echo "IndexNow accepted $url (HTTP $status)"
            ;;
        *)
            echo "IndexNow rejected $url (HTTP $status)" >&2
            if [[ -s "$response_file" ]]; then
                cat "$response_file" >&2
                echo >&2
            fi
            exit 1
            ;;
    esac

    rm -f "$response_file"
    trap - EXIT
done
