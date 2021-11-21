use argh::{from_env, FromArgs};
use k2o::*;
use std::{path::PathBuf, process::exit, thread::Builder};


/// Run an Auction bot in Twitch chat via the IRC Bridge.
#[derive(FromArgs)]
#[argh(note="\
A default Configuration file is written upon first execution. The generated
file must then be modified to supply valid Twitch authentication data before
a successful connection can be made.")]
struct Command {
    /// channels to be joined
    #[argh(positional, arg_name = "CHANNEL")]
    channels: Vec<String>,

    /// specify a path for the Config file
    #[argh(option, long = "config", arg_name = "PATH")]
    cfg_path: Option<PathBuf>,

    /// check for the existence of a Config file and exit
    #[argh(switch)]
    lsconf: bool,

    /// ensure the existence of a Config file and exit
    #[argh(switch)]
    mkconf: bool,

    /// check the Config file for validity and exit
    #[argh(switch)]
    okconf: bool,

    /// overwrite any existing Config file with the default values
    #[argh(switch)]
    reinit: bool,
}


fn main() {
    if let Err(..) = ctrlc::set_handler(|| {
        eprintln!("SIGINT");
        stop();
    }) {
        fatal!("Failed to set Interrupt Handler.");
        exit(1);
    }

    let Command {
        cfg_path, channels,
        lsconf, mkconf, okconf, reinit,
    } = from_env();

    if okconf {
        use config::ConfigError::ParseError;

        match match cfg_path {
            Some(path) => Config::from_path(path),
            None => Config::setup(),
        } {
            Ok(..) => println!("Config file is valid."),
            Err(ParseError(e)) => println!("Config file is NOT valid: {}", e),
            Err(e) => {
                err!("Failed to check Config file: {}", e);
                exit(1);
            }
        }

    } else if lsconf {
        exit(Config::find(cfg_path));

    } else if mkconf {
        exit(Config::ensure(cfg_path, reinit));

    } else if channels.is_empty() {
        err!("Provide at least one Channel to join.");
        exit(1);

    } else {
        match match cfg_path {
            Some(path) => Config::from_path(path),
            None => Config::setup(),
        } {
            Ok(config) => {
                let mut threads = Vec::with_capacity(channels.len());

                for channel in channels.into_iter() {
                    info!("Joining #{}...", &channel);
                    let cfg = config.clone();

                    match Builder::new()
                        .name(format!("#{}", channel))
                        .spawn(move || run_bot(channel, &cfg))
                    {
                        Err(error) => err!("Failed to spawn thread: {}", error),
                        Ok(thread) => threads.push(thread),
                    }
                }

                for thread in threads {
                    if let Err(err) = thread.join() {
                        err!("{:?}", err);
                    }
                }
            }
            Err(error) => {
                err!("{}", &error);
                exit(error.status());
            }
        }
    }
}
