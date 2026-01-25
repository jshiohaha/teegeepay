#!/bin/bash

set -a; source "$(dirname "$0")/../.env"; set +a

curl -X POST http://localhost:6767/api/tokens \
-H "Content-Type: application/json" \
-d "{
    \"name\": \"Cypher USD\",
    \"symbol\": \"cUSD\",
    \"decimals\": 9,
    \"mintKeypair\": \"${MINT_KP}\"
}"