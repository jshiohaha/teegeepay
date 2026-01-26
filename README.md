# Cypherpay

A Telegram mini app enabling confidential Solana transfers over Telegram. Built for hackathon purposes, this project demonstrates SPL Token-2022 confidential transfer capabilities through a user-friendly Telegram interface.

## Architecture

```
┌─────────────────┐      ┌──────────────┐      ┌─────────────────┐
│  Telegram User  │─────▶│  Next.js UI  │─────▶│   Rust API      │
│   (Mini App)    │      │   (Frontend) │      │   (Axum)        │
└─────────────────┘      └──────────────┘      └────────┬────────┘
                                                         │
                                                         ▼
                                                ┌─────────────────┐
                                                │  Solana Network │
                                                │  (SPL Token-22) │
                                                └─────────────────┘
```

The Next.js UI serves as a Telegram mini app that communicates with the Rust API backend. The API manages wallet keypairs, builds and executes Solana transactions using SPL Token-2022 with confidential transfer extensions.

**Important Security Note**: Since Telegram doesn't have an embedded wallet, this implementation stores user keypairs on the backend (non-custodial wallets managed server-side). This is **NOT** a secure production implementation and is intended for hackathon/demonstration purposes only.

## Features

- Confidential transfers via Solana address or Telegram handle
- Backend-managed wallets associated with Telegram handles
- SPL Token-2022 confidential transfer support
- PostgreSQL-backed transaction history

## Tech Stack

### Backend (crates/api)

- **Rust** with Axum web framework
- **SQLx** for database operations
- **Tokio** async runtime
- **Solana SDK 3.0** with SPL Token-2022
- **PostgreSQL** for data persistence

### Frontend (ui)

- **Next.js** (React framework)
- Telegram Mini App integration
- Modern UI components

### Blockchain

- **SPL Token-2022** with confidential transfer extensions
- No custom smart contracts required

## Prerequisites

- **Node.js 18+** and npm 10+
- **Rust 1.92+**
- **Solana CLI 2.3+**
- **Docker** and Docker Compose
- **pnpm** (for frontend package management)
- **Solana cluster with confidential transfers enabled** (e.g., local Surfpool instance)

## Project Structure

```
.
├── crates/
│   └── api/              # Rust API backend
│       ├── src/          # API source code
│       ├── migrations/   # Database migrations
│       ├── Dockerfile    # API container definition
│       └── .env.example  # Environment variable template
├── ui/                   # Next.js frontend
│   ├── app/             # Next.js app directory
│   ├── components/      # React components
│   ├── lib/             # Utility functions
│   └── styles/          # CSS/styling
├── scripts/             # Utility scripts
│   ├── api-local.sh     # Run API locally
│   ├── create-mint.sh   # Create token mint
│   ├── reset-network.sh # Reset Solana network
│   └── run-migrations.sh# Run database migrations
├── docker-compose.yml   # Docker services configuration
└── Cargo.toml          # Rust workspace configuration
```

## Setup

### 1. Environment Variables

Copy the example environment file:

```bash
cp crates/api/.env.example crates/api/.env
```

Configure the following variables in `crates/api/.env`:

```bash
DATABASE_URL=postgres://user:password@localhost:5432/solana_api
RUST_LOG=info
RPC_URL=http://localhost:8899
TELEGRAM_BOT_TOKEN=your_telegram_bot_token
JWT_SECRET=your_jwt_secret
AUTHORITY_KP=base58_encoded_keypair
AUDITOR_KP=base58_encoded_keypair
DEV_MODE=false
```

**Required Environment Variables:**

- `TELEGRAM_BOT_TOKEN`: Your Telegram bot API token
- `JWT_SECRET`: Secret key for JWT authentication
- `AUTHORITY_KP`: Base58-encoded authority keypair for mint operations
- `AUDITOR_KP`: Base58-encoded auditor keypair for confidential transfers
- `RPC_URL`: Solana RPC endpoint with confidential transfer support

### 2. UI Environment Variables

Configure the UI environment variables in `ui/.env.local` as needed.

### 3. Solana Network

Ensure you have a Solana cluster running with confidential transfer support. For local development, use Surfpool:

```bash
# Start a local Surfpool instance with confidential transfers enabled
surfpool start
```

Or connect to a devnet/testnet endpoint that supports SPL Token-2022.

## Development

### Start the Backend API

The API runs in Docker with PostgreSQL:

```bash
docker compose up --build
```

This will:

- Start PostgreSQL database
- Run database migrations
- Start the Rust API on port 6767

### Start the Frontend

In a separate terminal:

```bash
cd ui
pnpm install
pnpm run dev
```

The UI will be available at `http://localhost:3000`.

## Utility Scripts

The `scripts/` directory contains helpful utilities:

- `api-local.sh` - Run the API outside of Docker for development
- `create-mint.sh` - Create a new token mint with confidential transfers
- `reset-network.sh` - Reset the Solana network state
- `run-migrations.sh` - Manually run database migrations
- `db-up.sh` - Start only the database container
- `container-down.sh` - Stop all containers

## API Endpoints

The API runs on port 6767 and provides endpoints for:

- User authentication via Telegram
- Wallet management
- Confidential transfer operations
- Transaction history

## Important Notes

- This implementation is for **hackathon/demonstration purposes only**
- Wallets are stored on the backend, which is **not secure** for production
- Requires a Solana cluster with SPL Token-2022 and confidential transfer support
- For production use, implement proper key management and custody solutions

## License

MIT License - Provided "as-is" without warranty of any kind.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
