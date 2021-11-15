use potassium_oxide as k2o;
use std::{env::args, process::exit, thread::spawn};


macro_rules! die {
    ($($msg:tt)*) => {|_err| {
        k2o::fatal!($($msg)*);
        std::process::exit(1);
    }};
}


fn main() {
    // ctrlc::set_handler(k2o::stop).expect("Failed to set Interrupt Handler");
    ctrlc::set_handler(k2o::stop).unwrap_or_else(
        die!("Failed to set Interrupt Handler.")
    );
    // let config = k2o::Config::setup().unwrap();

    match k2o::Config::setup() {
        Ok(config) => {
            let channels = args().skip(1);

            let mut threads = Vec::with_capacity(channels.len());

            for channel in channels {
                eprintln!("Joining {}...", channel);
                let cfg = config.clone();
                threads.push(spawn(move || k2o::run_bot(channel, &cfg)));
            }

            for thread in threads {
                if let Err(err) = thread.join() {
                    eprintln!("{:?}", err);
                }
            }
        }
        Err(error) => {
            eprintln!("{}", &error);
            exit(error.status());
        }
    }
}
