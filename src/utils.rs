use std::time;

/// Get UNIX time in microseconds
pub fn micros_since_epoch() -> i64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64
}

/// Convert Windows NT time to UNIX time
pub fn nt_to_unix(time: i64) -> i64 {
    const NT_UNIX_MICROSEC_DIFF: i64 = 11_644_473_600_000_000;
    time - NT_UNIX_MICROSEC_DIFF
}
