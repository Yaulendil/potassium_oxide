#[macro_use]
extern crate serde;

#[macro_use]
mod macros;

pub mod bot;
pub mod config;

pub use bot::{Bot, BotExit};
pub use config::Config;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};


static STOP: AtomicBool = AtomicBool::new(false);


pub fn running() -> bool { !STOP.load(SeqCst) }
pub fn stop() { STOP.store(true, SeqCst); }


pub fn run_bot(channel: String, config: &Config) {
    match Bot::new(channel, &config) {
        Err(failed) => err!("Failed to run bot: {:?}", failed),
        Ok(mut bot) => match bot.run() {
            Err(failed) => err!("Failed to run bot: {}", failed),
            Ok(_) => info!("Complete."),
        }
    }
}
