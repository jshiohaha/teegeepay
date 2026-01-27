#!/bin/bash

set -a; source "$(dirname "$0")/../.env.local"; set +a

cargo run -p solana-api