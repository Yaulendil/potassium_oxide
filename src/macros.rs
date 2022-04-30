#[macro_export]
macro_rules! usd {
    () => { "USD" };
    ($amount:literal) => { concat!("$", $amount) };
    ($amount:expr) => { format_args!(usd!("{}"), $amount) };
}


cfg_if::cfg_if! {
    if #[cfg(not(any(
        feature = "currency_cad",
        feature = "currency_eur",
        feature = "currency_gbp",
        feature = "currency_jpy",
    )))] {
        #[macro_export]
        macro_rules! money {($($tt:tt)*) => {$crate::usd!($($tt)*)}}
    } else if #[cfg(all(
        feature = "currency_cad",
        not(any(
            feature = "currency_eur",
            feature = "currency_gbp",
            feature = "currency_jpy",
        )),
    ))] {
        #[macro_export]
        macro_rules! money {
            () => { "CAD" };
            ($amount:literal) => { concat!("$", $amount) };
            ($amount:expr) => { format_args!(money!("{}"), $amount) };
        }
    } else if #[cfg(all(
        feature = "currency_eur",
        not(any(
            feature = "currency_cad",
            feature = "currency_gbp",
            feature = "currency_jpy",
        )),
    ))] {
        #[macro_export]
        macro_rules! money {
            () => { "EUR" };
            ($amount:literal) => { concat!("€", $amount) };
            ($amount:expr) => { format_args!(money!("{}"), $amount) };
        }
    } else if #[cfg(all(
        feature = "currency_gbp",
        not(any(
            feature = "currency_cad",
            feature = "currency_eur",
            feature = "currency_jpy",
        )),
    ))] {
        #[macro_export]
        macro_rules! money {
            () => { "GBP" };
            ($amount:literal) => { concat!("£", $amount) };
            ($amount:expr) => { format_args!(money!("{}"), $amount) };
        }
    } else if #[cfg(all(
        feature = "currency_jpy",
        not(any(
            feature = "currency_cad",
            feature = "currency_eur",
            feature = "currency_gbp",
        )),
    ))] {
        #[macro_export]
        macro_rules! money {
            () => { "JPY" };
            ($amount:literal) => { concat!("¥", $amount) };
            ($amount:expr) => { format_args!(money!("{}"), $amount) };
        }
    } else {
        #[macro_export]
        macro_rules! money {($($tt:tt)*) => {$crate::usd!($($tt)*)}}
        compile_error!("Only one Currency feature may be enabled.");
    }
}


#[cfg(not(feature = "chrono"))]
#[macro_export]
macro_rules! _msg {
    (#RESET) => { "\x1B[m" };
    (#FATAL) => { "\x1B[1;93;41m" };
    (#WARN) => { "\x1B[33m" };
    (#ERR) => { "\x1B[91m" };
    (#DB) => { "\x1B[90m" };

    //  A string literal, potentially followed by formatting arguments.
    (@$macro:ident $fmt:tt $pre:tt: $text:literal $($tail:tt)*) => {
        $macro!(
            concat!(
                $crate::_msg!(#$fmt),
                concat!(stringify!($pre), ": ", $text),
                $crate::_msg!(#RESET),
            )
            $($tail)*
        )
    };

    //  Formatting arguments; Insert a pair of template braces.
    (@$macro:ident $fmt:tt $pre:tt: $($tail:tt)+) => {
        $crate::_msg!(@$macro $fmt $pre: "{}", $($tail)+)
    };
}

#[cfg(feature = "chrono")]
#[macro_export]
macro_rules! _msg {
    (#RESET) => { "\x1B[m" };
    (#FATAL) => { "\x1B[1;93;41m" };
    (#WARN) => { "\x1B[33m" };
    (#ERR) => { "\x1B[91m" };
    (#DB) => { "\x1B[90m" };

    //  A string literal, potentially followed by formatting arguments.
    (@$macro:ident $fmt:tt $pre:tt: $text:literal $($tail:tt)*) => {
        $macro!(
            concat!(
                $crate::_msg!(#$fmt),
                concat!("[{}] ", stringify!($pre), ": ", $text),
                $crate::_msg!(#RESET),
            ),
            ::chrono::Local::now().format($crate::TS_FMT)
            $($tail)*
        )
    };

    //  Formatting arguments; Insert a pair of template braces.
    (@$macro:ident $fmt:tt $pre:tt: $($tail:tt)+) => {
        $crate::_msg!(@$macro $fmt $pre: "{}", $($tail)+)
    };
}


#[macro_export]
macro_rules! fatal {($($text:tt)+) => {$crate::_msg!(@eprintln FATAL FATAL: $($text)+)}}
#[macro_export]
macro_rules! err {($($text:tt)+) => {$crate::_msg!(@eprintln ERR ERROR: $($text)+)}}
#[macro_export]
macro_rules! warn {($($text:tt)+) => {$crate::_msg!(@eprintln WARN WARN: $($text)+)}}
#[macro_export]
macro_rules! info {($($text:tt)+) => {$crate::_msg!(@eprintln DB INFO: $($text)+)}}
#[macro_export]
macro_rules! chat {($($text:tt)+) => {$crate::_msg!(@println RESET CHAT: $($text)+)}}
