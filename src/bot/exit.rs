use std::fmt::{Display, Formatter};


pub enum BotExit {
    BotExited,
    ConnectionClosed,
    ThreadPanic(Box<dyn std::any::Any + Send>),

    RunnerError(twitchchat::runner::Error),
    IoError(std::io::Error),
}

impl From<twitchchat::runner::Error> for BotExit {
    fn from(e: twitchchat::runner::Error) -> Self {
        Self::RunnerError(e)
    }
}

impl From<std::io::Error> for BotExit {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<BotExit> for String {
    fn from(err: BotExit) -> Self {
        err.to_string()
    }
}

impl Display for BotExit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BotExit::BotExited => f.write_str("Exited."),
            BotExit::ConnectionClosed => f.write_str("Connection closed."),
            BotExit::ThreadPanic(_) => f.write_str("Worker thread panicked."),
            // BotExit::ThreadPanic(e) => write!(f, "Worker thread panicked: {:?}", e),

            BotExit::RunnerError(e) => write!(f, "Twitch Chat error: {}", e),
            BotExit::IoError(e) => write!(f, "I/O Error: {}", e),
        }
    }
}
