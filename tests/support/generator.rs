use std::io::{self, Write};

pub const CLIENTS: u16 = 1_000;

// Deterministic and memory-free: each tx's client is a pure function of its
// ID, so dispute-family rows can reference any earlier tx with the right
// client without the generator remembering anything.
pub fn write_transactions<W: Write>(mut out: W, rows: u64) -> io::Result<()> {
    if rows >= u64::from(u32::MAX) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "row count must fit in the u32 transaction ID space",
        ));
    }
    writeln!(out, "type,client,tx,amount")?;
    let mut rng = XorShift(0x2545_f491_4f6c_dd1d);
    let mut next_tx: u32 = 1;
    for _ in 0..rows {
        let roll = rng.next() % 100;
        if roll < 90 || next_tx == 1 {
            let tx = next_tx;
            next_tx += 1;
            let kind = if roll < 70 { "deposit" } else { "withdrawal" };
            let units = rng.next() % 10_000_000 + 1;
            writeln!(
                out,
                "{kind},{},{tx},{}.{:04}",
                client_of(tx),
                units / 10_000,
                units % 10_000
            )?;
        } else {
            let tx = (rng.next() % u64::from(next_tx - 1)) as u32 + 1;
            let kind = match roll {
                90..=94 => "dispute",
                95..=97 => "resolve",
                _ => "chargeback",
            };
            writeln!(out, "{kind},{},{tx},", client_of(tx))?;
        }
    }
    out.flush()
}

fn client_of(tx: u32) -> u16 {
    (tx.wrapping_mul(2_654_435_761) % u32::from(CLIENTS)) as u16 + 1
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}
