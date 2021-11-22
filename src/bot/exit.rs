use std::fmt::{Display, Formatter, self};
use twitchchat::runner::Error as RunnerError;


pub enum BotExit {
    BotExited,
    ConnectionClosed,
    ThreadPanic,

    RunnerError(RunnerError),
    IoError(std::io::Error),
}

impl From<RunnerError> for BotExit {
    fn from(e: RunnerError) -> Self {
        Self::RunnerError(e)
    }
}

impl From<std::io::Error> for BotExit {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}


impl Display for BotExit {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BotExit::BotExited => f.write_str("Exited."),
            BotExit::ConnectionClosed => f.write_str("Connection closed."),
            BotExit::ThreadPanic => f.write_str("Worker thread panicked."),

            BotExit::RunnerError(e) => write!(f, "Twitch Chat error: {}", e),
            BotExit::IoError(e) => write!(f, "I/O Error: {}", e),
        }
    }
}
