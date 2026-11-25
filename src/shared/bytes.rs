//! Human-readable sizes.

const KB: u64 = 1024;
const MB: u64 = 1 << 20;
const GB: u64 = 1 << 30;

pub fn fmt_bytes(n: u64) -> String {
    if n >= GB {
        format!("{:.2} GB", n as f64 / GB as f64)
    } else if n >= MB {
        format!("{:.2} MB", n as f64 / MB as f64)
    } else {
        format!("{} KB", n.div_ceil(KB))
    }
}
