#!/bin/bash

set -a; source "$(dirname "$0")/../.env"; set +a

cargo run -p solana-api