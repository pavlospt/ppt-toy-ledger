# Scale and concurrency

How the engine behaves as inputs grow toward the full `u32` transaction-ID
space, and how it would be deployed behind many concurrent input streams.
Nothing here is implemented beyond what is stated; the aim is to make the
limits and the path forward explicit.

## What already scales

- **Input is streamed.** Rows are read, applied and dropped one at a time, so
  input size doesn't drive memory.
- **Per-stream cost is small.** Each reader holds csv's 8 KiB buffer and two
  reused record buffers. Accounts are bounded at 65 536 (`u16` client IDs).
- **The engine is a plain state machine.** `Engine::apply` takes one
  `Transaction`, does no I/O, never blocks, and an `Engine` can move to another
  thread (`engine::tests::concurrency::engine_can_move_to_another_thread`).
  Any loop that produces transactions can drive it.

## The limit: the deposit store

Any accepted deposit may be disputed later, and the input has no dispute
window, so every accepted deposit is kept: amount, client and state, keyed by
transaction ID. Each map entry is 16 bytes plus one control byte, and the map
doubles in size as it grows.

| Stored deposits | Map memory (estimate) | Measured peak |
|---|---|---|
| 10 M | ~285 MB | 573 MB (includes growth) |
| 1 B | ~36 GB | — |
| 2³² ≈ 4.3 B (every `u32` ID) | ~146 GB, ~219 GB while growing | — |

No in-memory layout fits the full ID space on one machine. Even a perfectly
dense 11-byte record per ID would need about 47 GB. Withdrawals don't count
toward this; only deposits are stored.

## Options for bounding it

Each changes behaviour or adds infrastructure, so each is a decision, not a
tweak.

| Option | Effect | Cost |
|---|---|---|
| Dispute window | Drop deposits older than N transactions or N days; a later dispute becomes `UnknownTx`. Memory is bounded by the window. | Needs a policy the input doesn't define, and an ordering or timestamp to measure age. |
| Disk-backed store | Keep recent deposits in memory and spill older ones to an embedded key-value store. Memory is bounded; disputes of old deposits cost a disk read. | A storage dependency and its operational care. |
| Pre-sizing | When the input size is known, size the map once and skip the growth peak (about 1.5× → 1×). | Only helps when the size is known up front. |

## Many concurrent streams

There are two deployment models, depending on what the streams mean.

### Independent ledgers (one per stream)

- Run one `Engine` per connection, inside that connection's task.
- No state is shared, so there are no locks.
- Memory is the sum of each stream's deposit store, which is where the options
  above apply.
- The csv reader is synchronous. In an async server, parse on a blocking
  thread, or with an async CSV reader, and hand `Transaction`s to the engine.

### One shared ledger fed by many streams

Split the state by client:

```text
stream 1 ─┐                       ┌─▶ shard 0: Engine (clients where id % N == 0)
stream 2 ─┼─ parse ─ route by ────┼─▶ shard 1: Engine (clients where id % N == 1)
  …       │          client id    │     …
stream k ─┘                       └─▶ shard N-1
```

- Connection tasks parse rows and send each `Transaction` over a bounded
  channel to the shard that owns its client. The bound gives backpressure all
  the way back to TCP.
- Each shard owns one `Engine`, so there are no locks.
- This works because a client can only act on its own deposits, so every
  dispute, resolve and chargeback lands on the shard that holds the deposit.
- Each client's rows from one stream stay in order. Rows for the same client
  arriving on different streams have no defined order anyway.

Trade-offs to decide before building it:

- **Duplicate deposit IDs across shards go undetected.** Each shard only sees
  its own deposits. Catching them needs a shared ID set, or IDs that are
  unique per shard.
- **A cross-client dispute becomes `UnknownTx` instead of `ClientMismatch`.**
  The row lands on the disputing client's shard, which never stored the
  deposit. Both are ignored silently, so nothing visible changes.
- **Shards share the memory budget.** The store options above still apply,
  per shard.

## Why none of this is built yet

Clarity comes first: each of these designs adds code that reviewers must read,
and each needs a policy decision (window length, storage, routing) that the
input format doesn't settle. The engine is already shaped so that any of them
can wrap it without changes to `Engine` itself.
