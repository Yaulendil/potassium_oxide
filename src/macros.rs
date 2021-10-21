macro_rules! currency {
    () => { "USD" };
    ($amount:expr) => { format_args!("${}", $amount) };
}
