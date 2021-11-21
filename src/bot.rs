mod auction;
mod client;

use auction::*;
use client::Client;
use crate::config::Config;
use humantime::{format_duration, FormattedDuration};
use parking_lot::Mutex;
use smol::{block_on, Timer};
use std::{
    sync::Arc,
    thread::{Builder, current, sleep},
    time::{Duration, Instant},
};
use twitchchat::{
    connector,
    messages::{Commands, Privmsg},
    runner::AsyncRunner,
    RunnerError,
    Status,
    twitch::UserConfigError,
    UserConfig,
};


fn substring_to_end<'a>(main: &'a str, sub: &str) -> Option<&'a str> {
    let valid = main.as_bytes().as_ptr_range();

    if !sub.is_empty() && valid.contains(&sub.as_ptr()) {
        let idx = unsafe { sub.as_ptr().offset_from(valid.start) } as usize;

        Some(&main[idx..])
    } else {
        None
    }
}


fn auction_check(lock: &mut Option<Auction>) -> Option<String> {
    match lock {
        Some(auction) => match auction.remaining() {
            Some(time) => match time.as_secs() + 1 {
                t @ 1..=5 => Some(format!("Auction: {}...", t)),

                t @ (10 | 15 | 30 | 60 // <=1m
                | 120 | 300 | 600 | 900 | 1800 | 3600 // <=1h
                | 7200 | 10800) => match auction.get_bid() {
                    Some(Bid { amount, .. }) => Some(format!(
                        "Auction: {} seconds remain. The current bid is {}.",
                        t, usd!(amount),
                    )),
                    None => Some(format!("Auction: {} seconds remain.", t)),
                }

                _ => None,
            }
            None => {
                let out: Option<String> = match auction.get_bid() {
                    Some(Bid { amount, bidder }) => Some(format!(
                        "The Auction has been won by @{}, with a bid of {}.",
                        bidder, usd!(amount),
                    )),
                    None => Some("The Auction has ended with no bids.".into()),
                };

                lock.take();
                out
            }
        }
        None => None,
    }
}


#[derive(Debug)]
pub enum BotExit {
    ConnectionClosed,
    BotExited,
    ClientErr,
    ConfigErr,

    RunnerError(RunnerError),
    IoError(std::io::Error),
    ThreadPanic,
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

impl From<BotExit> for String {
    fn from(err: BotExit) -> Self {
        format!("{:?}", err)
    }
}


pub struct Bot<'b> {
    channel: String,
    config: &'b Config,
    client: Option<Client>,
    auction: Arc<Mutex<Option<Auction>>>,
}

impl<'b> Bot<'b> {
    pub fn new(channel: String, config: &'b Config) -> Self {
        Self { channel, config, client: None, auction: Default::default() }
    }

    fn authenticate(&self, msg: &Privmsg<'_>) -> bool {
        self.config.is_admin(msg.name())
            || msg.is_broadcaster()
            || msg.is_moderator()
    }

    fn should_ignore(&self, msg: &Privmsg<'_>) -> bool {
        self.config.is_blacklisted(msg.name())
    }

    pub async fn send(&self, msg: impl AsRef<str>) {
        if let Some(mut client) = self.client.clone() {
            client.send(msg).await.ok();
        }
    }

    pub fn run(&mut self) -> Result<(), String> {
        match self.config.get_auth() {
            Ok(conf) => block_on(async move {
                while crate::running() {
                    info!("Bot closed: {:?}", self.run_once(&conf).await);

                    if crate::running() {
                        let delay: Duration = self.config.get_reconnect();
                        info!("Reconnecting in {}s.\n", delay.as_secs());
                        Timer::after(delay).await;
                    }
                }

                Ok(())
            }),
            Err(err) => Err(String::from(match err {
                UserConfigError::InvalidName => "Invalid Username",
                UserConfigError::InvalidToken => "Invalid OAuth Token",
                UserConfigError::PartialAnonymous => "Partial Anonymous login",
                _ => "Unknown error",
            })),
        }
    }

    async fn run_once(&mut self, uconf: &UserConfig) -> Result<BotExit, BotExit> {
        info!("Connecting...");
        let connection = connector::smol::Connector::twitch()?;
        let mut runner = AsyncRunner::connect(connection, uconf).await?;
        info!("Connected.");

        let client = Client::new(self.channel.clone(), &mut runner).await?;

        let auction_loop = {
            let mut cli: Client = client.clone();
            let auction: Arc<Mutex<Option<Auction>>> = self.auction.clone();
            let subname: String = format!("#{}/auctions", self.channel);

            Builder::new().name(subname).spawn(move || {
                /// Interval between Auction updates.
                const INTERVAL: Duration = Duration::from_secs(1);
                /// Timeout period to try locking the Auction Mutex.
                const TO: Duration = Duration::from_millis(
                    (INTERVAL.as_millis() / 2) as _
                );

                let mut time = Instant::now();

                while crate::running() {
                    if let Some(mut lock) = auction.try_lock_for(TO) {
                        if let Some(text) = auction_check(&mut lock) {
                            if let Err(e) = block_on(cli.send(text)) {
                                err!(
                                    "Error on Auction thread {:?}: {}",
                                    current().name().unwrap_or_default(), e,
                                );
                                break;
                            }
                        }
                    }

                    time += INTERVAL;
                    sleep(time.saturating_duration_since(Instant::now()));
                }

                block_on(cli.quit());
            })?
        };

        self.client = Some(client);
        let result = self.main_loop(runner).await;
        self.client = None;

        match auction_loop.join() {
            Err(_e) => Err(BotExit::ThreadPanic),
            Ok(()) => Ok(result),
        }
    }

    async fn main_loop(&mut self, mut runner: AsyncRunner) -> BotExit {
        loop {
            match runner.next_message().await {
                Ok(Status::Message(msg)) => self.handle_message(msg).await,
                Ok(Status::Quit) => break BotExit::BotExited,
                Ok(Status::Eof) => break BotExit::ConnectionClosed,
                Err(error) => break error.into(),
            }
        }
    }

    async fn handle_command(
        &mut self,
        msg: &Privmsg<'_>,
        words: &[&str],
    ) -> Option<String> {
        let author: &str = msg.display_name().unwrap_or_else(|| msg.name());
        let usr_op: bool = self.authenticate(msg);

        match words {
            ["auction", "status", ..] => {
                let lock = self.auction.lock();
                let auction: &Auction = lock.as_ref()?;
                let time: FormattedDuration = format_duration(
                    auction.remaining().unwrap_or_default(),
                );

                Some(match auction.get_bid() {
                    None => format!(
                        "The Auction still has {} remaining. The minimum bid \
                        is {}, but there have not been any bids yet.",
                        time,
                        usd!(auction.get_minimum()),
                    ),
                    Some(Bid { amount, bidder }) => format!(
                        "The Auction still has {} remaining. The leader is \
                        currently {}, who bids {}.",
                        time,
                        bidder,
                        usd!(amount),
                    ),
                })
            }
            ["auction", subcom, args @ ..] if usr_op => match *subcom {
                "start" => {
                    let mut lock = self.auction.lock();

                    if lock.is_some() {
                        Some(format!("An Auction is already running; Invoke '{}\
                        auction stop' to cancel it.", self.config.get_prefix()))
                    } else {
                        let channel = msg.channel().trim_start_matches('#');
                        let mut dur = self.config.get_duration(channel);
                        let mut hlm = self.config.get_helmet(channel);
                        let mut max = self.config.get_max_raise(channel);
                        let mut min = self.config.get_min_bid(channel);
                        let mut tok = args.iter();

                        while let Some(flag) = tok.next() {
                            match *flag {
                                "-d" | "-t" => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        dur = Duration::from_secs(vl);
                                    }
                                }
                                "-h" => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        hlm = Duration::from_secs(vl);
                                    }
                                }
                                "-r" => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        max = vl;
                                    }
                                }
                                "-m" => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        min = vl;
                                    }
                                }
                                _ => {}
                            }
                        }

                        let new: &mut Auction = lock.insert(Auction::new(
                            dur, hlm, max, min,
                        ));

                        Some(new.explain(self.config.get_prefix()))
                    }
                }
                "stop" => Some(match self.auction.lock().take() {
                    Some(..) => "Auction stopped.".into(),
                    None => "No Auction is currently running.".into(),
                }),
                _ => None,
            }
            ["bid", arg, ..] => match substring_to_end(msg.data(), arg)
                .unwrap_or(arg).trim_start_matches('$').parse::<usize>()
            {
                Ok(bid) => Some(match self.auction.lock()
                    .as_mut()?
                    .bid(&author, bid)
                {
                    BidResult::Ok => format!(
                        "NEW BID: @{} has bid {}.",
                        author, usd!(bid),
                    ),
                    BidResult::RepeatBidder => format!(
                        "@{}: You are already the top bidder.",
                        author,
                    ),
                    BidResult::AboveMaximum(max) => format!(
                        "@{}: You can only raise by a maximum of {}.",
                        author, usd!(max),
                    ),
                    BidResult::BelowMinimum(min) => format!(
                        "@{}: The minimum bid is {}.",
                        author, usd!(min),
                    ),
                    BidResult::DoesNotRaise(cur) => format!(
                        "@{}: The current bid is {}.",
                        author, usd!(cur),
                    ),
                }),
                Err(..) => Some(format!(
                    "@{}: A bid must be a whole number of USD.",
                    author,
                )),
            }
            #[cfg(debug_assertions)]
            ["echo", arg, ..] => Some(format!(
                "{} said: {:?}",
                author, substring_to_end(msg.data(), arg).unwrap_or(arg),
            )),
            _ => None,
        }
    }

    async fn handle_message(&mut self, message: Commands<'_>) {
        use Commands::*;

        if let Some(reply) = match message {
            Privmsg(msg) if !self.should_ignore(&msg)
            => if let Some(line) = msg.data().strip_prefix(self.config.get_prefix()) {
                println!("[{}] {}: {}", msg.channel(), msg.name(), msg.data());

                let words: Vec<&str> = line.split_whitespace().collect();

                self.handle_command(&msg, &words).await
            } else {
                None
            }

            // Raw(_) => {}
            //
            // IrcReady(_) => {}
            // Ready(_) => {}
            // Cap(_) => {}
            //
            // ClearChat(_) => {}
            // ClearMsg(_) => {}
            // GlobalUserState(_) => {}
            // HostTarget(_) => {}
            // Join(_) => {}
            // Notice(_) => {}
            // Part(_) => {}
            // Ping(_) => {}
            // Pong(_) => {}
            // Reconnect(_) => {}
            // RoomState(_) => {}
            // UserNotice(_) => {}
            // UserState(_) => {}
            // Whisper(_) => {}

            _ => None
        } {
            self.send(reply).await;
        }
    }
}
