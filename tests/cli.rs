#[path = "support/generator.rs"]
mod generator;

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;

const HEADER: &str = "client,available,held,total,locked";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn ledger_binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ppt-ledger"))
}

// Output row order is unspecified, so rows are compared as a sorted set with
// whitespace around fields ignored.
fn normalize(csv: &str) -> (String, Vec<Vec<String>>) {
    let mut lines = csv.lines().filter(|line| !line.trim().is_empty());
    let header = lines.next().unwrap_or_default().replace(' ', "");
    let mut rows: Vec<Vec<String>> = lines
        .map(|line| line.split(',').map(|f| f.trim().to_string()).collect())
        .collect();
    rows.sort();
    (header, rows)
}

#[track_caller]
fn assert_fixture(name: &str) -> String {
    let output = ledger_binary()
        .arg(fixture(&format!("{name}.csv")))
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        output.status.success(),
        "{name}: exit {:?}, stderr:\n{stderr}",
        output.status
    );
    let actual = String::from_utf8(output.stdout).unwrap();
    let expected = fs::read_to_string(fixture(&format!("{name}.expected.csv"))).unwrap();
    assert_eq!(
        normalize(&actual),
        normalize(&expected),
        "{name}: stdout was\n{actual}"
    );
    assert_eq!(actual.lines().next(), Some(HEADER), "{name}: header");
    stderr
}

fn units(value: &str) -> i128 {
    value.replace('.', "").parse().unwrap()
}

mod scenarios {
    use super::*;

    #[test]
    fn basic() {
        assert_fixture("basic");
    }

    #[test]
    fn whitespace_and_trailing_commas() {
        assert_fixture("whitespace_and_trailing_commas");
    }

    #[test]
    fn insufficient_funds() {
        assert_fixture("insufficient_funds");
    }

    #[test]
    fn dispute_resolve() {
        assert_fixture("dispute_resolve");
    }

    #[test]
    fn dispute_chargeback_lock() {
        assert_fixture("dispute_chargeback_lock");
    }

    #[test]
    fn locked_account_rejections() {
        assert_fixture("locked_account_rejections");
    }

    #[test]
    fn dispute_negative_available() {
        assert_fixture("dispute_negative_available");
    }

    #[test]
    fn invalid_disputes() {
        assert_fixture("invalid_disputes");
    }

    #[test]
    fn malformed_rows() {
        assert_fixture("malformed_rows");
    }

    #[test]
    fn duplicate_tx() {
        assert_fixture("duplicate_tx");
    }

    #[test]
    fn precision() {
        assert_fixture("precision");
    }

    #[test]
    fn header_only() {
        assert_fixture("header_only");
    }

    #[test]
    fn reordered_columns() {
        assert_fixture("reordered_columns");
    }

    #[test]
    fn account_creation() {
        assert_fixture("account_creation");
    }
}

mod diagnostics {
    use super::*;

    #[test]
    fn malformed_rows_are_reported_once_each_with_row_numbers() {
        let stderr = assert_fixture("malformed_rows");
        let lines: Vec<_> = stderr.lines().collect();
        assert_eq!(lines.len(), 11, "stderr:\n{stderr}");
        for row in 3..=13 {
            let prefix = format!("row {row}:");
            assert!(
                lines.iter().any(|l| l.contains(&prefix)),
                "missing {prefix} in stderr:\n{stderr}"
            );
        }
    }

    #[test]
    fn engine_rejections_are_reported_with_row_numbers() {
        let stderr = assert_fixture("insufficient_funds");
        assert!(stderr.contains("row 3:"), "{stderr}");
        assert!(stderr.contains("insufficient"), "{stderr}");
    }
}

mod fatal_errors {
    use super::*;

    #[test]
    fn missing_argument_is_a_usage_error() {
        let output = ledger_binary().output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage"));
    }

    #[test]
    fn extra_arguments_are_a_usage_error() {
        let output = ledger_binary()
            .arg(fixture("basic.csv"))
            .arg(fixture("basic.csv"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn unreadable_file_exits_with_failure() {
        let output = ledger_binary()
            .arg(fixture("does_not_exist.csv"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("does_not_exist.csv"));
    }

    #[test]
    fn bad_header_exits_with_failure() {
        let output = ledger_binary()
            .arg(fixture("bad_header.csv"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn empty_file_exits_with_failure() {
        let output = ledger_binary()
            .arg(fixture("empty_file.csv"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }
}

// Run with `cargo nextest run --release --run-ignored only -E 'test(large_input)'`.
#[test]
#[ignore = "slow: streams a million generated rows through the binary"]
fn large_input_streams_successfully() {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("large_input.csv");
    generator::write_transactions(BufWriter::new(File::create(&path).unwrap()), 1_000_000).unwrap();

    let output = ledger_binary().arg(&path).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let (header, rows) = normalize(&stdout);
    assert_eq!(header, HEADER);
    assert!(!rows.is_empty());
    assert!(rows.len() <= usize::from(generator::CLIENTS));
    for row in rows {
        assert_eq!(units(&row[3]), units(&row[1]) + units(&row[2]), "{row:?}");
    }
}
