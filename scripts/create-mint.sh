#!/bin/bash

set -a; source "$(dirname "$0")/../.env"; set +a

curl -X POST $API_BASE_URL/api/tokens \
-H "Content-Type: application/json" \
-d "{
    \"name\": \"TeeGee USD\",
    \"symbol\": \"tgUSD\",
    \"uri\": \"https://arweave.net/nFo9Nwcam4ek0SwtKQchYD47T9dkTpGqL62CgcXSjZE\",
    \"decimals\": 9,
    \"mintKeypair\": \"${MINT_KP}\"
}"