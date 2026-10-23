# Future work

Known limitations and possible next steps. None of these are implemented.

## Many concurrent streams

Run one engine per stream, or split one shared ledger by client across shard
workers. [scale.md](scale.md) describes both models and their trade-offs.

## Bounding the deposit store

Memory grows with the number of accepted deposits, at about 28–57 bytes each
(see [performance.md](performance.md)). Covering every `u32` transaction ID
won't fit in memory. [scale.md](scale.md) has the projections and compares
the options: a dispute window, a disk-backed store, and pre-sizing the map.

## Detecting duplicate withdrawal IDs

Withdrawal IDs aren't stored, so a repeated withdrawal ID goes undetected
([behaviour.md](behaviour.md)). A compact set of seen IDs, such as a compressed
bitmap, would catch it at a small memory cost.

## Operations

Replace the free-text stderr lines with structured logs, plus counters of
rejected rows by reason.

## Packaging

Split the crate into a library and a thin binary, so other front ends (a
network service, for example) can reuse the engine without copying it.
