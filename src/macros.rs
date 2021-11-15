#[macro_export]
macro_rules! usd {
    () => { "USD" };
    ($amount:literal) => { concat!("$", $amount) };
    ($amount:expr) => { format_args!(usd!("{}"), $amount) };
}


#[macro_export]
macro_rules! _msg {
    (#RESET) => { "\x1B[m" };
    (#FATAL) => { "\x1B[1;93;41m" };
    (#WARN) => { "\x1B[33m" };
    (#ERR) => { "\x1B[91m" };

    //  A string literal message.
    (@$fmt:tt $pre:tt: $text:literal) => {
        eprintln!(concat!(
            $crate::_msg!(#$fmt),
            stringify!($pre), ": ", $text,
            $crate::_msg!(#RESET),
        ));
    };

    //  A template literal, followed by formatting arguments.
    (@$fmt:tt $pre:tt: $text:literal, $($tail:tt)+) => {
        eprintln!(concat!(
            $crate::_msg!(#$fmt),
            stringify!($pre), ": ", $text,
            $crate::_msg!(#RESET),
        ), $($tail)+);
    };

    // //  Formatting arguments; Insert a pair of template braces.
    // (@$fmt:tt $pre:tt: $($tail:tt)+) => {
    //     eprintln!(concat!(
    //         $crate::_msg!(#$fmt),
    //         stringify!($pre), ": {}",
    //         $crate::_msg!(#RESET),
    //     ), $($tail)+);
    // };

    //  Formatting arguments; Insert a pair of template braces.
    (@$fmt:tt $pre:tt: $($tail:tt)+) => {_msg!(@$fmt $pre: "{}", $($tail)+)};
}

#[macro_export]
macro_rules! fatal {($($text:tt)+) => {$crate::_msg!(@FATAL FATAL: $($text)+)}}
#[macro_export]
macro_rules! err {($($text:tt)+) => {$crate::_msg!(@ERR ERROR: $($text)+)}}
#[macro_export]
macro_rules! warn {($($text:tt)+) => {$crate::_msg!(@WARN WARNING: $($text)+)}}
#[macro_export]
macro_rules! info {($($text:tt)+) => {$crate::_msg!(@RESET INFO: $($text)+)}}
