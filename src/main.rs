#[macro_use]
mod macros;
mod bot;

use bot::Bot;
use std::{
    env::args,
    thread::spawn,
    sync::atomic::{AtomicBool, Ordering::SeqCst},
};


static STOP: AtomicBool = AtomicBool::new(false);


fn running() -> bool { !STOP.load(SeqCst) }
fn stop() { STOP.store(true, SeqCst); }


fn run_bot(channel: String) {
    let mut bot = Bot::new(channel);
    bot.run()
}


fn main() {
    ctrlc::set_handler(stop).expect("Failed to set Interrupt Handler");

    let channels = args().skip(1);
    let mut threads = Vec::with_capacity(channels.len());

    for channel in channels {
        eprintln!("Joining {}...", channel);
        threads.push(spawn(move || run_bot(channel)));
    }

    for thread in threads {
        if let Err(err) = thread.join() {
            eprintln!("{:?}", err);
        }
    }
}
