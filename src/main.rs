use potassium_oxide::*;
use std::{env::args, process::exit, thread::spawn};


fn main() {
    ctrlc::set_handler(stop).unwrap_or_else(|_| {
        fatal!("Failed to set Interrupt Handler.");
        exit(1);
    });

    match Config::setup() {
        Ok(config) => {
            let channels = args().skip(1);

            let mut threads = Vec::with_capacity(channels.len());

            for channel in channels {
                info!("Joining #{}...", &channel);
                let cfg = config.clone();
                threads.push(spawn(move || run_bot(channel, &cfg)));
            }

            for thread in threads {
                if let Err(err) = thread.join() {
                    err!("{:?}", err);
                }
            }
        }
        Err(error) => {
            fatal!("{}", &error);
            exit(error.status());
        }
    }
}
