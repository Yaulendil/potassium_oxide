mod run;

use std::{path::PathBuf, process::exit};
use argh::{from_env, FromArgs};


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
        k2o::stop();
    }) {
        k2o::fatal!("Failed to set Interrupt Handler: {}", e);
        exit(1);
    }

    let Command { cfg_path, channels, check_cfg, make_cfg } = from_env();

    if check_cfg {
        run::cfg_check(cfg_path);
    } else if make_cfg {
        run::cfg_make(cfg_path);
    } else {
        run::bot(cfg_path, channels);
    }

    // unreachable!()
}
