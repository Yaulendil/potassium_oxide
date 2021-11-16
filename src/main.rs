use potassium_oxide::*;
use std::{env::args, process::exit, thread::spawn};


fn main() {
    ctrlc::set_handler(stop).unwrap_or_else(|_| {
        fatal!("Failed to set Interrupt Handler.");
        exit(1);
    });

    let mut config_path: Option<String> = None;
    let channels: Vec<String> = args().skip(1).collect();

    //  TODO: Command line flags.

    match match config_path {
        Some(path) => Config::from_path(path),
        None => Config::setup(),
    } {
        Ok(config) => {
            let mut threads = Vec::with_capacity(channels.len());

            for channel in channels.into_iter() {
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
