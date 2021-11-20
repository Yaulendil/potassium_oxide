mod auction;
mod client;

use auction::*;
use client::Client;
use crate::config::Config;
use humantime::{format_duration, FormattedDuration};
use parking_lot::Mutex;
use smol::{block_on, Timer};
use std::{sync::Arc, thread::{sleep, spawn}, time::{Duration, Instant}};
use twitchchat::{
    connector,
    messages::{Commands, Privmsg},
    runner::AsyncRunner,
    RunnerError,
    Status,
    twitch::UserConfigError,
    UserConfig,
};


const RECON_DELAY: Duration = Duration::from_secs(10);


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


fn contains<I, T, U>(sequence: I, want: U) -> bool where
    I: IntoIterator<Item=T>,
    T: PartialEq<U>,
{
    sequence.into_iter().any(|item: T| item == want)
}


#[derive(Debug)]
pub enum BotExit {
    ConnectionClosed,
    BotExited,
    ClientErr,
    ConfigErr,

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
    pub fn new(channel: String, config: &'b Config) -> Result<Self, BotExit> {
        Ok(Self { channel, config, client: None, auction: Default::default() })
    }

    fn authenticate(&self, msg: &Privmsg<'_>) -> bool {
        contains(&self.config.bot.admins, &msg.name())
            || msg.is_broadcaster()
            || msg.is_moderator()
    }

    fn should_ignore(&self, msg: &Privmsg<'_>) -> bool {
        contains(&self.config.bot.blacklist, &msg.name())
    }

    pub async fn send(&self, msg: impl AsRef<str>) {
        if let Some(mut client) = self.client.clone() {
            client.send(msg).await.unwrap();
        }
    }

    pub fn run(&mut self) -> Result<(), String> {
        match UserConfig::builder()
            .name(&self.config.auth.username)
            .token(&self.config.auth.oauth)
            .enable_all_capabilities()
            .build()
        {
            Ok(conf) => block_on(async move {
                while crate::running() {
                    info!("Bot closed: {:?}", self.run_once(&conf).await);

                    if crate::running() {
                        info!("Reconnecting in {}s.\n", RECON_DELAY.as_secs());
                        Timer::after(RECON_DELAY).await;
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

        let client = Client::new(self.channel.clone(), &mut runner).await;

        let auction_loop = {
            let mut cli: Client = client.clone();
            let arc: Arc<Mutex<Option<Auction>>> = self.auction.clone();

            spawn(move || {
                const INC: Duration = Duration::from_secs(1);

                let mut time = Instant::now();

                while crate::running() {
                    if let Some(mut lock) = arc.try_lock_until(time) {
                        if let Some(text) = auction_check(&mut lock) {
                            if let Err(e) = block_on(cli.send(text)) {
                                err!("Error on Auction thread: {}", e);
                                break;
                            }
                        }
                    }

                    time += INC;
                    sleep(time.saturating_duration_since(Instant::now()));
                }

                block_on(cli.quit());
            })
        };

        self.client = Some(client);
        let result = self.main_loop(runner).await;
        self.client = None;

        auction_loop.join().expect("Failed to rejoin Auction thread.");
        println!();

        Ok(result)
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
                        auction stop' to cancel it.", self.config.bot.prefix))
                    } else {
                        let channel = msg.channel().trim_start_matches('#');
                        let mut sec = self.config.default_duration(channel);
                        let mut min = self.config.default_minimum(channel);
                        let mut max = self.config.raise_limit(channel);
                        let mut hlm = self.config.helmet(channel);
                        let mut tok = args.iter();

                        while let Some(flag) = tok.next() {
                            match *flag {
                                "-h" => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        hlm = vl;
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
                                "-t" => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        sec = vl;
                                    }
                                }
                                _ => {}
                            }
                        }

                        let new: &mut Auction = lock.insert(Auction::new(
                            Duration::from_secs(sec),
                            Duration::from_secs(hlm),
                            max,
                            min,
                        ));

                        Some(new.explain(&self.config.bot.prefix))
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
            => if let Some(line) = msg.data().strip_prefix(&self.config.bot.prefix) {
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
