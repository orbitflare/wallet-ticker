# wallet-ticker

Live Solana wallet dashboard in less than 150 LOC. Pass a pubkey, get a terminal view of its SOL balance and every SPL token account it owns, updating in place as the balances change on-chain.

Example project for the [OrbitFlare Rust SDK](https://crates.io/crates/orbitflare-sdk). Uses the `rpc` feature to bootstrap current state and the `ws` feature to stream live updates.

## Running

```bash
ORBITFLARE_LICENSE_KEY=ORBIT-XXXXXX-NNNNNN-MMMMMM cargo run --release -- <pubkey>
```

The SDK picks the license key up from `ORBITFLARE_LICENSE_KEY` automatically. Replace `<pubkey>` with any Solana address you want to watch.

Sample output:

```
wallet: CKs1E69a2e9TmH4mKKLrXFF8kD3ZnwKjoEuXa6sz9WqX

  SOL     0.040420950
  EPjF...Dt1v      42.500000
  So11...1112       0.005000
```

Ctrl+C to quit.

## How it works

On startup, the RPC client runs `get_balance` plus `get_token_accounts_by_owner` to fetch the wallet's current state. Each account then gets its own `account_subscribe` WebSocket subscription, spawned concurrently, and the updates fan into a single `mpsc` channel that the main loop drains and renders.
