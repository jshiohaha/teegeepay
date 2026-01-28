#!/bin/bash

set -a; source "$(dirname "$0")/../.env"; set +a

curl -X POST $API_BASE_URL/api/tokens \
-H "Content-Type: application/json" \
-d "{
    \"name\": \"TeeGee USD\",
    \"symbol\": \"tgUSD\",
    \"uri\": \"$METADATA_URL\",
    \"decimals\": 9,
    \"mintKeypair\": \"${MINT_KP}\"
}"