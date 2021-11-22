mod auction;
mod client;
mod exit;

use auction::*;
use client::{Client, Response};
use crate::ConfigFile;
pub use exit::BotExit;
use humantime::{format_duration, FormattedDuration};
use parking_lot::Mutex;
use smol::{block_on, Timer};
use spin_sleep::sleep;
use std::{
    sync::Arc,
    thread::{Builder, current},
    time::{Duration, Instant},
};
use twitchchat::{
    connector::smol::Connector,
    messages::{Commands, Privmsg},
    runner::AsyncRunner,
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

impl From<BotExit> for String {
    fn from(err: BotExit) -> Self {
        format!("{}", err)
    }
}


pub struct Bot {
    channel: String,
    config: ConfigFile,
    client: Option<Client>,
    auction: Arc<Mutex<Option<Auction>>>,
    stopped: Option<Instant>,
}

impl Bot {
    pub fn new(channel: String, config: ConfigFile) -> Self {
        Self {
            channel,
            config,
            client: None,
            auction: Default::default(),
            stopped: None,
        }
    }

    fn authenticate(&self, msg: &Privmsg<'_>) -> bool {
        self.config.is_admin(msg.name())
            || msg.is_broadcaster()
            || msg.is_moderator()
    }

    fn should_ignore(&self, msg: &Privmsg<'_>) -> bool {
        self.config.is_blacklisted(msg.name())
    }

    pub fn run(&mut self) -> Result<(), String> {
        match self.config.get_auth() {
            Ok(conf) => block_on(async move {
                while crate::running() {
                    match self.run_once(&conf).await {
                        Ok(status) => info!("Bot closed: {}", status),
                        Err(status) => warn!("Bot closed: {}", status),
                    }

                    if crate::running() {
                        let delay: Duration = self.config.get_reconnect();
                        info!("Reconnecting in {}s.\n", delay.as_secs());
                        Timer::after(delay).await;
                        self.config.reload().ok();
                    }
                }

                Ok(())
            }),
            Err(err) => Err(match err {
                UserConfigError::InvalidName => "Invalid Username",
                UserConfigError::InvalidToken => "Invalid OAuth Token",
                UserConfigError::PartialAnonymous => "Partial Anonymous login",
                _ => "Unknown error",
            }.into()),
        }
    }

    async fn run_once(&mut self, uconf: &UserConfig) -> Result<BotExit, BotExit> {
        info!("Connecting...");
        let connection = Connector::twitch()?;
        let mut runner = AsyncRunner::connect(connection, uconf).await?;
        info!("Connected.");

        let mut client = Client::new(self.channel.clone(), &mut runner).await?;

        if let Some(stopped) = self.stopped.take() {
            if let Some(mut lock) = self.auction.try_lock() {
                if let Some(auction) = lock.as_mut() {
                    let downtime = Instant::now() - stopped;

                    auction.add_time(downtime);

                    let status = match auction.get_bid() {
                        Some(Bid { amount, bidder }) => format!(
                            "The highest bidder is currently @{} at {}",
                            bidder, usd!(amount),
                        ),
                        None => format!(
                            "The minimum bid is {}",
                            usd!(auction.get_minimum()),
                        ),
                    };

                    let time = format_duration(match auction.remaining() {
                        Some(time) => Duration::from_secs(time.as_secs() + 1),
                        None => Duration::from_secs(0),
                    });

                    client.send(format!(
                        "Sorry, it seems I lost connection for a moment. No \
                        problem though, I can continue the Auction from where \
                        it left off. {}, with {} remaining.",
                        status, time,
                    )).await?;
                }
            }
        }

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

                while crate::running() && cli.is_running() {
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
        self.stopped = Some(Instant::now());

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
    ) -> Option<Response> {
        use Response::*;

        let author: &str = msg.display_name().unwrap_or_else(|| msg.name());
        let usr_op: bool = self.authenticate(msg);

        match words {
            ["auction", "status", ..] => {
                let lock = self.auction.lock();
                let auction: &Auction = lock.as_ref()?;
                let time: FormattedDuration = format_duration(
                    auction.remaining().unwrap_or_default(),
                );

                Some(Reply(match auction.get_bid() {
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
                }))
            }
            ["auction", subcom, args @ ..] if usr_op => match *subcom {
                "start" => {
                    let mut lock = self.auction.lock();

                    if lock.is_some() {
                        Some(Reply(format!(
                            "An Auction is already running; Invoke '{}auction \
                            stop' to cancel it.",
                            self.config.get_prefix(),
                        )))
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

                        Some(Message(new.explain(
                            &self.config.get_prefix(),
                            &self.config.get_verb(channel),
                        )))
                    }
                }
                "stop" => Some(Reply(match self.auction.lock().take() {
                    Some(..) => "Auction stopped.",
                    None => "No Auction is currently running.",
                }.into())),
                _ => None,
            }
            ["bid", arg, ..] => match substring_to_end(msg.data(), arg)
                .unwrap_or(arg).trim_start_matches('$').parse::<usize>()
            {
                Ok(bid) => Some(match self.auction.lock()
                    .as_mut()?
                    .bid(&author, bid)
                {
                    BidResult::Ok => Message(format!(
                        "NEW BID: @{} has bid {}.",
                        author, usd!(bid),
                    )),
                    BidResult::RepeatBidder => Reply(
                        "You are already the top bidder.".into()
                    ),
                    BidResult::AboveMaximum(max) => Reply(format!(
                        "You can only raise by a maximum of {}.",
                        usd!(max),
                    )),
                    BidResult::BelowMinimum(min) => Reply(format!(
                        "The minimum bid is {}.",
                        usd!(min),
                    )),
                    BidResult::DoesNotRaise(cur) => Reply(format!(
                        "The current bid is {}.",
                        usd!(cur),
                    )),
                }),
                Err(..) => Some(Reply(
                    "A bid must be a whole number of USD.".into()
                )),
            }
            #[cfg(debug_assertions)]
            ["die", ..] if usr_op => {
                self.client.as_ref()?.clone().quit().await;
                None
            }
            #[cfg(debug_assertions)]
            ["echo", arg, ..] => Some(Message(format!(
                "{} said: {:?}",
                author, substring_to_end(msg.data(), arg).unwrap_or(arg),
            ))),
            ["reload", ..] => Some(Reply(match self.config.reload() {
                Ok(..) => "Configuration reloaded.",
                Err(_) => "Failed to reload Config.",
            }.into())),
            _ => None,
        }
    }

    async fn handle_message(&mut self, message: Commands<'_>) {
        use Commands::*;

        match message {
            Privmsg(msg) if !self.should_ignore(&msg)
            => if let Some(words) = self.find_command(msg.data()) {
                chat!("({}) {}: {:?}", msg.channel(), msg.name(), msg.data());

                if let Some(reply) = self.handle_command(&msg, &words).await {
                    if let Some(client) = &mut self.client {
                        if let Err(err) = client.respond(&msg, reply).await {
                            warn!("Failed to send message: {}", err);
                        }
                    }
                }
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

            _ => {}
        }
    }

    fn find_command<'s>(&self, text: &'s str) -> Option<Vec<&'s str>> {
        match text.strip_prefix(self.config.get_prefix()) {
            Some(line) => Some(line.split_whitespace().collect()),
            None => None,
        }
    }
}
