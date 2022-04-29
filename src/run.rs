//! Runner functions to be called from `main.rs`. These were previously inline
//!     in `main()`, and are not part of `lib.rs`.

use std::{path::PathBuf, process::exit, thread::Builder};
use k2o::{
    {err, info, warn},
    config::{Config, ConfigFind, ConfigOpen},
    run_bot,
};


pub fn cfg_check(cfg_path: Option<PathBuf>) -> ! {
    use ConfigFind::*;
    use ConfigOpen::*;

    match Config::find(cfg_path) {
        DoesNotExist(path) => {
            println!("Config file at {} does not exist.", path.display());
            exit(1);
        }
        Exists(path, FileInaccessible(e)) => {
            println!("Config file at {} inaccessible: {}", path.display(), e);
            exit(1);
        }
        Exists(path, FileInvalid(e)) => {
            println!("Config file at {} invalid: {}", path.display(), e);
            exit(1);
        }
        Exists(path, FileValid(..)) => {
            println!("Valid Config file found: {}", path.display());
            exit(0);
        }
        NoPath => {
            println!("Cannot find Config filepath.");
            exit(1);
        }
    }
}


pub fn cfg_make(cfg_path: Option<PathBuf>) -> ! {
    match Config::find(cfg_path).path() {
        Some(path) => match Config::create(&path, true) {
            Ok(..) => {
                println!("Default Config file created: {}", path.display());
                exit(0);
            }
            Err(e) => {
                println!(
                    "Failed to write {} as default Config file: {}",
                    path.display(), e,
                );
                exit(1);
            }
        }
        None => {
            println!("Cannot find Config filepath.");
            exit(1);
        }
    }
}


pub fn bot(cfg_path: Option<PathBuf>, channels: Vec<String>) -> ! {
    if channels.is_empty() {
        err!("Provide at least one Channel to join.");
        exit(1);
    }

    info!("Starting Potassium Oxide v{}", env!("CARGO_PKG_VERSION"));

    let (path, open): (PathBuf, ConfigOpen) = match Config::find(cfg_path) {
        ConfigFind::Exists(path, open) => {
            info!("Using existing Config file: {}", path.display());
            (path, open)
        }
        ConfigFind::DoesNotExist(path) => match Config::create(&path, true) {
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
        }
        ConfigFind::NoPath => {
            err!("Cannot find Config file path.");
            exit(1);
        }
    };

    match open {
        ConfigOpen::FileInaccessible(e) => {
            err!("Config file at {} inaccessible: {}", path.display(), e);
            exit(1);
        }
        ConfigOpen::FileInvalid(e) => {
            err!("Config file at {} invalid: {}", path.display(), e);
            exit(1);
        }
        ConfigOpen::FileValid(config) => {
            let mut threads = Vec::with_capacity(channels.len());
            let config = config.with_path(path);

            for channel in channels.into_iter() {
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

            exit(0);
        }
    }
}
