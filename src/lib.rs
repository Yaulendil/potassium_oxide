#[macro_use]
extern crate serde;

#[macro_use]
mod macros;

pub mod bot;
pub mod config;
pub mod saving;

use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
pub use bot::{Bot, BotExit};
pub use config::{Config, ConfigFile};
use saving::AuctionFinished;


static STOP: AtomicBool = AtomicBool::new(false);

/// Template string for timestamps. Format defined by [`chrono`].
///
/// [`chrono`]: https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html
#[cfg(feature = "chrono")]
pub const TS_FMT: &str = "%Y-%m-%d %H:%M:%S %z";


pub fn running() -> bool { !STOP.load(SeqCst) }
pub fn stop() { STOP.store(true, SeqCst); }


pub fn run_bot(channel: String, config: ConfigFile) {
    match Bot::new(channel, config).run() {
        Err(failed) => err!("Failed to run bot: {}", failed),
        Ok(..) => info!("Complete."),
    }
}
