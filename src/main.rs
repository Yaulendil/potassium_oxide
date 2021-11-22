use argh::{from_env, FromArgs};
use k2o::{*, config::{ConfigFind::*, ConfigOpen::{*, self}}};
use std::{path::PathBuf, process::exit, thread::Builder};


/// Run an Auction bot in Twitch chat via the IRC Bridge.
#[derive(FromArgs)]
#[argh(note = "\
A default Configuration file is written upon first execution, or when run
with `--mkconf`. The generated file must then be modified to supply valid
Twitch authentication data before any successful connections can be made.")]
struct Command {
    /// channels to be joined
    #[argh(positional, arg_name = "CHANNEL")]
    channels: Vec<String>,

    /// specify a path for the Config file
    #[argh(option, long = "cfg", arg_name = "PATH")]
    cfg_path: Option<PathBuf>,

    /// check the Config file for validity and exit
    #[argh(switch, long = "cfg-check")]
    check_cfg: bool,

    /// create a new default Config file and exit
    #[argh(switch, long = "mkconf")]
    make_cfg: bool,
}


fn main() {
    if let Err(e) = ctrlc::set_handler(|| {
        eprintln!("Interrupted");
        stop();
    }) {
        fatal!("Failed to set Interrupt Handler: {}", e);
        exit(1);
    }

    let Command { cfg_path, channels, check_cfg, make_cfg } = from_env();

    if check_cfg {
        exit(match Config::find(cfg_path) {
            DoesNotExist(path) => {
                println!("Config file at {} does not exist.", path.display());
                1
            }
            Exists(path, FileInaccessible(e)) => {
                println!("Config file at {} inaccessible: {}", path.display(), e);
                1
            }
            Exists(path, FileInvalid(e)) => {
                println!("Config file at {} invalid: {}", path.display(), e);
                1
            }
            Exists(path, FileValid(..)) => {
                println!("Valid Config file found: {}", path.display());
                0
            }
            NoPath => {
                println!("Cannot find Config filepath.");
                1
            }
        });
    } else if make_cfg {
        exit(match Config::find(cfg_path).path() {
            Some(path) => match Config::create(&path, true) {
                Ok(..) => {
                    println!("Default Config file created: {}", path.display());
                    0
                }
                Err(e) => {
                    println!(
                        "Failed to write {} as default Config file: {}",
                        path.display(), e,
                    );
                    1
                }
            }
            None => {
                println!("Cannot find Config filepath.");
                1
            }
        })
    } else {
        if channels.is_empty() {
            err!("Provide at least one Channel to join.");
            exit(1);
        }

        let (path, open): (PathBuf, ConfigOpen) = match Config::find(cfg_path) {
            Exists(path, open) => {
                info!("Using existing Config file: {}", path.display());
                (path, open)
            }
            DoesNotExist(path) => match Config::create(&path, true) {
                Ok(..) => {
                    info!("Default Config file created: {}", path.display());
                    let open = Config::open(&path);
                    (path, open)
                }
                Err(e) => {
                    err!(
                        "Failed to write {} as default Config file: {}",
                        path.display(), e,
                    );
                    exit(1);
                }
            },
            NoPath => {
                err!("Cannot find Config file path.");
                exit(1);
            }
        };

        match open {
            FileInaccessible(e) => {
                err!("Config file at {} inaccessible: {}", path.display(), e);
                exit(1);
            }
            FileInvalid(e) => {
                err!("Config file at {} invalid: {}", path.display(), e);
                exit(1);
            }
            FileValid(config) => {
                let mut threads = Vec::with_capacity(channels.len());
                let config = config.with_path(path);

                for channel in channels.into_iter() {
                    info!("Joining #{}...", &channel);
                    let cfg = config.clone();

                    match Builder::new()
                        .name(format!("#{}", channel))
                        .spawn(move || run_bot(channel, cfg))
                    {
                        Err(error) => warn!("Failed to spawn thread: {}", error),
                        Ok(thread) => threads.push(thread),
                    }
                }

                for thread in threads {
                    if let Err(err) = thread.join() {
                        err!("{:?}", err);
                    }
                }
            }
        }
    }
}
