# Prompt history

The prompts that shaped this project, in order, one entry each. Short prompts
are quoted; longer ones are summarised. Approvals, push requests and other
housekeeping are left out, and so is anything copied from the original brief.

1. **Task and phased plan.** Build a Rust CLI that streams a CSV of
   transactions through an account engine and prints final balances as CSV.
   Work in five phases, stopping for approval after each: plan, type
   skeletons, tests first, implementation one slice at a time, polish.
   Constraints: a binary crate run as `cargo run -- transactions.csv >
   accounts.csv`; common crates only; clean `cargo fmt` and `clippy`; no panics
   on bad input; small commits; nothing in the repository that points to where
   the task came from. (The specification it restated is omitted.)
2. **Plan amendments.** A binary only, with no library; comments only where
   they explain a decision; tests written first, then iterate with the
   implementation until they pass.
3. **Toolchain.** "set it to rust version 1.99.1"
4. **Performance.** "investigate if there is a serde SIMD implementation so
   that we can make it faster", then apply the reusable trim-buffer fix the
   investigation found.
5. **Audit, comments, nextest, docs.** Audit the repository for bloat and ask
   before removing anything; comment the state machine; run tests with
   nextest; move the README's detail into agent-oriented docs. Then drop
   `assert_cmd` and slim `proptest`.
6. **Clean code and resource use.** Prompted by two points from the brief
   (omitted): readability refactors, a plain-loop amount parser, compact
   deposit entries, scale and concurrency docs, and row numbers in
   diagnostics.
7. **Naming.** "align the naming to directory and everywhere to
   ppt-toy-ledger"
8. **CI.** Pin actions to commit SHAs and run every test under both
   `cargo test` (the default) and nextest, with nextest documented in a
   collapsible README section.
9. **Deposit layout.** "Update the commit where we introduced repr(C) to also
   have comments about why we went that way"
10. **This file.** Add a prompt history, keep it free of the task's origin,
    and list only prompts that produced work, briefly.
