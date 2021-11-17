use argh::{from_env, FromArgs};
use k2o::*;
use std::{path::PathBuf, process::exit, thread::spawn};


/**
Run an Auction bot in Twitch chat via the IRC Bridge.

Exit Status:
  0 :: Success.
  _ :: // TODO
*/ //  NOTE: Block comment is necessary here to properly layout help text.
#[derive(FromArgs)]
struct Command {
    /// channels to be joined
    #[argh(positional, arg_name = "CHANNEL")]
    channels: Vec<String>,

    /// specify a path for the Config file
    #[argh(option, long = "config", arg_name = "PATH")]
    cfg_path: Option<PathBuf>,

    /// ensure the existence of a Config file and exit
    #[argh(switch)]
    mkconf: bool,

    /// overwrite any existing Config file with the default values
    #[argh(switch)]
    reinit: bool,
}


fn main() {
    if let Err(..) = ctrlc::set_handler(stop) {
        fatal!("Failed to set Interrupt Handler.");
        exit(1);
    }

    let Command { cfg_path, channels, mkconf, reinit } = from_env();

    if mkconf {
        exit(Config::ensure(cfg_path, reinit));
    }

    if channels.is_empty() {
        err!("Provide at least one Channel to join.");
        exit(1);
    }

    match match cfg_path {
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
