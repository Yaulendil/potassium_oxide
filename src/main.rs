mod bot;

use bot::Bot;
use std::{
    env::args,
    thread::spawn,
};


fn run_bot(channel: String) {
    let mut bot = Bot::new(channel);
    bot.run()
}


fn main() {
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
