pub mod db;
pub mod web;
pub mod zip;

/// Creates a "timestamp" which is the number of milliseconds since UNIX_EPOCH.
/// Casts from u128 to u64 for convenience.
pub fn timestamp() -> u64 {
    wasm_timer::SystemTime::now()
        .duration_since(wasm_timer::UNIX_EPOCH).unwrap()
        .as_millis() as u64
}
