# Performance

Measured numbers, where the time goes, and how to reproduce the measurements.
Read this before changing `src/io.rs`, `src/amount.rs` or the maps in
`src/engine.rs`.

## Hot-path rules

- **Stream; never load the whole input.**
- **No allocation per row:**
  - `TransactionReader` reuses two `StringRecord` buffers, `record` and
    `trimmed`, for the whole stream.
  - Fields are trimmed by hand into `trimmed`. csv's own trimming
    (`Trim::All`) allocates a new record per row, which made the program
    about 2× slower.
- **Decode without copying.** `RawRecord` borrows `&str` fields from the
  buffer. The only per-row allocation is the `String` inside an
  `UnknownType` error.
- **Store only what disputes need.** One `Deposit` entry (amount, client,
  state) per accepted deposit; nothing else grows with the input.
  `Deposit` is `repr(C, packed(4))`, so a map entry is 16 bytes instead of 24.
  A test pins that size.

## Measurements

Release build on an Apple Silicon laptop, `/usr/bin/time -l`, median of three
runs:

| Input | Time | Peak memory |
|---|---|---|
| 10 M generated mixed rows | 1.3 s | 38 MB |
| 10 M deposits, all accepted and stored (worst case for memory) | 1.5 s | 573 MB |

Earlier versions, on the same inputs:

| Change | Mixed | Deposits |
|---|---|---|
| Original (csv trims each row, 24-byte entries) | 3.9 s, 55 MB | 3.9 s, 841 MB |
| Reused trim buffer | 1.9 s, 55 MB | 1.7 s, 841 MB |
| 16-byte deposit entries | 1.9 s, 38 MB | 1.5 s, 573 MB |
| Engine rejections no longer logged (current) | 1.3 s, 38 MB | 1.5 s, 573 MB |

- **Mixed input:** chargebacks lock most accounts early, so most later
  deposits are rejected and never stored. That keeps its memory low. It used
  to write about 8.7 M stderr lines, one per rejection; it now writes none.
- **Worst case:**
  - The measured peak is about 57 bytes per stored deposit.
  - Each map entry is 16 bytes plus one control byte, in power-of-two tables,
    so memory should settle at about 28 bytes per deposit. This is an
    estimate.
  - Projections to the full `u32` ID space are in [scale.md](scale.md).
  - The peak is roughly double because growing the map briefly holds both
    the old and new tables.

## Where the time goes

From the `sample` profiler on the 10 M deposits input, after the trimming
change and before the 16-byte entry change (which mainly cut memory):

| Component | Share |
|---|---|
| Amount parsing and field validation | ~23% |
| HashMap (SipHash, growth) | ~22% |
| csv record handling and UTF-8 checks | ~19% |
| csv-core parsing | ~11% |
| serde decoding | ~9% |
| Engine logic | ~9% |

**Conclusions:**
- **A SIMD CSV parser would gain at most about 11%.** The fields are tiny, and
  simd-csv's own benchmarks show little gain on files like this. It also
  doesn't support whitespace trimming, rows of varying length, or serde.
- **The hasher is deliberately not swapped.** Transaction IDs are untrusted
  input, and SipHash resists hash flooding.

## Reproducing

```sh
cargo build --release
cargo run --release --example generate -- 10000000 > large.csv   # mixed input
/usr/bin/time -l ./target/release/ppt-toy-ledger large.csv > /dev/null 2>/dev/null
```

For the worst-case input (10 M deposits):

```sh
awk 'BEGIN{print "type,client,tx,amount"; for(i=1;i<=10000000;i++) printf "deposit,%d,%d,%d.%04d\n", i%1000+1, i, i%997+1, i%10000}' > large-deposits.csv
```

For a profile on macOS, run `sample <pid> 2` while the binary is running.
`.gitignore` excludes `/large*.csv`.
