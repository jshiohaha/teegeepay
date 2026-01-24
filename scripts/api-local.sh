DATABASE_URL=postgres://user:password@localhost:5432/solana_api \
    RUST_LOG=info \
    RPC_URL=http://localhost:8899 \
    TELEGRAM_BOT_TOKEN=token \
    JWT_SECRET=WsEWWfZez207luDwVRvxeE7M2x2JslLR4cqJFoI5uoN \
    AUTHORITY_KP=4e1YUcpmUrAZV8kwfNP2VC62p71MzAiF5xsjdV6hp77iHUBUewV3N5JwoPaoBeJKYU2s6FQas2zSG7kbiqNVhpxV \
    AUDITOR_KP=4pATUBtizivqSJeBFRPi4wcEzK86GYFNJUcpg5WQJQfxtxrgbmmDmueRt259nY55etwKftQp2cWUQ7fzdTVUVbvB \
    cargo run -p solana-api