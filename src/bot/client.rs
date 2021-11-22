use std::sync::{Arc, atomic::{AtomicBool, Ordering::SeqCst}};
use twitchchat::{
    commands as cmd,
    messages::Privmsg,
    runner::{AsyncRunner, NotifyHandle},
    RunnerError,
    writer::{AsyncWriter, MpscWriter},
};


#[derive(PartialEq)]
pub enum Response {
    Message(String),
    Reply(String),
}

impl Response {
    pub async fn send(
        self,
        msg: &Privmsg<'_>,
        w: &mut AsyncWriter<MpscWriter>,
    ) -> std::io::Result<()> {
        let channel = msg.channel();

        macro_rules! as_msg {
            ($text:expr) => {w.encode(cmd::privmsg(channel, $text))};
        }

        match self {
            Self::Message(text) => as_msg!(&text).await,
            Self::Reply(text) => match msg.tags().get("id") {
                Some(id) => w.encode(cmd::reply(channel, id, &text)).await,
                None => as_msg!(&text).await,
            }
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Message(text) => text,
            Self::Reply(text) => text,
        }
    }
}


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

    pub async fn respond(
        &mut self,
        msg: &Privmsg<'_>,
        reply: Response,
    ) -> std::io::Result<()> {
        if self.is_running() {
            chat!("(-> #{}) {:?}", &self.channel, reply.text());
            reply.send(msg, &mut self.writer).await
        } else {
            chat!("(-| #{}) {:?}", &self.channel, reply.text());
            warn!("Cannot send message: Client is closed.");
            Ok(())
        }
    }

    pub async fn send(&mut self, text: impl AsRef<str>) -> std::io::Result<()> {
        if self.is_running() {
            chat!("(-> #{}) {:?}", &self.channel, text.as_ref());
            self.writer.encode(cmd::privmsg(&self.channel, text.as_ref())).await
        } else {
            chat!("(-| #{}) {:?}", &self.channel, text.as_ref());
            warn!("Cannot send message: Client is closed.");
            Ok(())
        }
    }

    pub async fn quit(self) -> bool {
        self.running.swap(false, SeqCst) && self.handle_quit.notify().await
    }
}
