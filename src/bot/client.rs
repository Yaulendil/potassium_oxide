use std::sync::{Arc, atomic::{AtomicBool, Ordering::SeqCst}};
use twitchchat::{
    commands::privmsg,
    runner::{AsyncRunner, NotifyHandle},
    RunnerError,
    writer::{AsyncWriter, MpscWriter},
};


#[derive(Clone)]
pub struct Client {
    channel: String,
    running: Arc<AtomicBool>,
    handle_quit: NotifyHandle,
    writer: AsyncWriter<MpscWriter>,
}


impl Client {
    pub async fn new(
        channel: String,
        runner: &mut AsyncRunner,
    ) -> Result<Self, RunnerError> {
        runner.join(&channel).await?;

        Ok(Self {
            channel,
            running: Arc::new(AtomicBool::new(true)),
            handle_quit: runner.quit_handle(),
            writer: runner.writer(),
        })
    }

    pub fn is_running(&self) -> bool {
        self.running.load(SeqCst)
    }

    pub async fn send(&mut self, msg: impl AsRef<str>) -> std::io::Result<()> {
        if self.is_running() {
            chat!("(-> #{}) {:?}", &self.channel, msg.as_ref());
            self.writer.encode(privmsg(&self.channel, msg.as_ref())).await
        } else {
            warn!("Failed to send message: Client is closed.");
            chat!("(-| #{}) {:?}", &self.channel, msg.as_ref());
            Ok(())
        }
    }

    pub async fn quit(self) -> bool {
        self.running.swap(false, SeqCst) && self.handle_quit.notify().await
    }
}
