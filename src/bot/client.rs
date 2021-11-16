use twitchchat::{
    commands,
    runner::{AsyncRunner, NotifyHandle},
    writer::{AsyncWriter, MpscWriter},
};


#[derive(Clone)]
pub struct Client {
    channel: String,
    handle_quit: NotifyHandle,
    writer: AsyncWriter<MpscWriter>,
}


impl Client {
    pub async fn new(channel: String, runner: &mut AsyncRunner) -> Self {
        let _ = runner.join(&channel).await.unwrap();

        Self {
            channel,
            handle_quit: runner.quit_handle(),
            writer: runner.writer(),
        }
    }

    pub async fn send(&mut self, msg: impl AsRef<str>) -> smol::io::Result<()> {
        self.writer.encode(commands::privmsg(&self.channel, msg.as_ref())).await
    }

    pub async fn quit(self) -> bool {
        self.handle_quit.notify().await
    }
}
