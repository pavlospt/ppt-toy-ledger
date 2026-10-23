//! Writes a synthetic transaction CSV to stdout for load and memory checks:
//! `cargo run --release --example generate -- 10000000 > large.csv`

#[path = "../tests/support/generator.rs"]
mod generator;

use std::io::{self, BufWriter};

fn main() -> io::Result<()> {
    let rows = match std::env::args().nth(1) {
        Some(arg) => arg.parse().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "row count must be a number")
        })?,
        None => 1_000_000,
    };
    generator::write_transactions(BufWriter::new(io::stdout().lock()), rows)?;
    eprintln!(
        "generated {rows} rows across {} clients",
        generator::CLIENTS
    );
    Ok(())
}
