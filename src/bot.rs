pub mod auction;
mod client;
mod exit;
mod util;

use std::{
    sync::{Arc, atomic::{AtomicBool, Ordering::SeqCst}},
    thread::{Builder, current},
    time::{Duration, Instant},
};
use humantime::{format_duration, FormattedDuration};
use parking_lot::Mutex;
use smol::{block_on, Timer};
use spin_sleep::sleep;
use twitchchat::{
    connector::smol::Connector,
    messages::{Commands, Privmsg},
    runner::AsyncRunner,
    Status,
    twitch::UserConfigError,
    UserConfig,
};
use crate::ConfigFile;
use auction::{Auction, Bid, BidResult};
use client::{Client, Response};
pub use exit::BotExit;
pub use util::{is_quoted, split_cmd, substring_to_end, to_end_unquoted, unquote};


/// Define the values of remaining time at which an update on the auction should
///     be posted to chat automatically.
const fn announce_time(sec: u64) -> bool {
    match sec {
        10 | 15 | 30 | 60 // <=1m
        | 120 | 300 | 600 | 900 | 1800 | 3600 // <=1h
        => true,
        _ if 0 == sec % 3600 => {
            match sec / 3600 {
                1..=24 // <=1d
                | 36 | 48 | 72 // <=3d
                //  Beyond this, people can just use the status command. This
                //      may already be too far.
                => true,
                _ => false,
            }
        }
        _ => false,
    }
}


enum AuctionStatus {
    Active(Option<String>),
    Ended(String, Auction),
    Inactive,
}


fn auction_check(lock: &mut Option<Auction>) -> AuctionStatus {
    use AuctionStatus::*;

    match lock {
        Some(auction) => match auction.remaining() {
            Some(time) => match time.as_secs() + 1 {
                t @ 1..=5 => Active(Some(format!("Auction: {}...", t))),

                t if announce_time(t) => match auction.last_bid() {
                    Some(Bid { amount, .. }) => Active(Some(format!(
                        "Auction: {} seconds remain. The current bid{} is {}.",
                        t, auction.for_prize(), usd!(amount),
                    ))),
                    None => Active(Some(match auction.prize.as_ref() {
                        Some(prize) => format!(
                            "Auction: {} seconds remain to bid for {}.",
                            t, prize,
                        ),
                        None => format!("Auction: {} seconds remain.", t),
                    })),
                }

                _ => Active(None),
            }
            None => {
                let out: String = match auction.last_bid() {
                    Some(Bid { amount, bidder, .. }) => format!(
                        "The {} has been won by @{}, with a bid of {}.",
                        auction.describe(), bidder, usd!(amount),
                    ),
                    None => format!(
                        "The {} has ended with no bids.",
                        auction.describe(),
                    ),
                };

                info!("Auction finished.");
                Ended(out, lock.take().unwrap())
            }
        }
        None => Inactive,
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
        self.config.is_admin(msg.name(), msg.channel().trim_start_matches('#'))
            || msg.is_broadcaster()
            || msg.is_moderator()
    }

    fn should_ignore(&self, msg: &Privmsg<'_>) -> bool {
        self.config.is_blacklisted(msg.name(), msg.channel().trim_start_matches('#'))
    }

    pub fn run(&mut self) -> Result<(), String> {
        use UserConfigError::*;

        match self.config.auth() {
            Ok(conf) => block_on(async move {
                while crate::running() {
                    match self.run_once(&conf).await {
                        Ok(status) => info!("Bot closed: {}", status),
                        Err(status) => warn!("Bot closed: {}", status),
                    }

                    if crate::running() {
                        self.config.reload().ok();
                        let delay: Duration = self.config.reconnect();
                        info!("Reconnecting in {}s.\n", delay.as_secs());
                        Timer::after(delay).await;
                        self.config.reload().ok();
                    }
                }

                Ok(())
            }),
            Err(err) => Err(match err {
                InvalidName => String::from("Invalid Username"),
                InvalidToken => String::from("Invalid OAuth Token"),
                PartialAnonymous => String::from("Partially Anonymous login"),
                _ => format!("Unknown error: {err:?}"),
            }),
        }
    }

    async fn run_once(&mut self, uconf: &UserConfig) -> Result<BotExit, BotExit> {
        info!("Joining #{}...", self.channel);
        let run_thread = Arc::new(AtomicBool::new(true));
        let connection = Connector::twitch()?;
        let mut runner = AsyncRunner::connect(connection, uconf).await?;
        let mut client = Client::new(self.channel.clone(), &mut runner).await?;
        info!("Connected to #{}.", self.channel);

        if let Some(stopped) = self.stopped.take() {
            if let Some(mut lock) = self.auction.try_lock() {
                if let Some(auction) = lock.as_mut() {
                    let downtime = Instant::now() - stopped;

                    auction.add_time(downtime);

                    let status = match auction.last_bid() {
                        Some(Bid { amount, bidder, .. }) => format!(
                            "The highest bidder is currently @{} at {}",
                            bidder, usd!(amount),
                        ),
                        None => format!(
                            "The minimum bid is {}",
                            usd!(auction.min_bid),
                        ),
                    };

                    let time = format_duration(match auction.remaining() {
                        Some(time) => Duration::from_secs(time.as_secs() + 1),
                        None => Duration::from_secs(0),
                    });

                    client.send(format!(
                        "Sorry, it seems I lost connection for a moment. No \
                        problem though, I can continue the {} from where it \
                        left off. {}, with {} remaining.",
                        auction.describe(), status, time,
                    )).await?;
                }
            }
        }

        let auction_thread = {
            let mut cli: Client = client.clone();
            let auction: Arc<Mutex<Option<Auction>>> = self.auction.clone();
            let running: Arc<AtomicBool> = run_thread.clone();

            let channel: String = self.channel.clone();
            let subname: String = format!("#{}/auctions", channel);
            let summary: bool = self.config.summary(&channel);

            #[cfg(feature = "csv")]
            let opt_csv = self.config.file_csv().map(|p| p.to_owned());

            Builder::new().name(subname).spawn(move || {
                /// Interval between Auction updates.
                const INTERVAL: Duration = Duration::from_secs(1);
                /// Timeout period to try locking the Auction Mutex.
                const TIMEOUT: Duration = Duration::from_millis(500);

                let mut time = Instant::now();

                while crate::running() && cli.is_running() && running.load(SeqCst) {
                    if let Some(mut lock) = auction.try_lock_for(TIMEOUT) {
                        let status = auction_check(&mut lock);

                        if let AuctionStatus::Active(Some(text))
                        | AuctionStatus::Ended(text, _) = &status {
                            if let Err(e) = block_on(cli.send(text)) {
                                err!(
                                    "Error on Auction thread {:?}: {}",
                                    current().name().unwrap_or_default(), e,
                                );
                                break;
                            }
                        }

                        if let AuctionStatus::Ended(_, auct) = status {
                            if summary {
                                let finished = auct.finish();

                                if let Err(e) = finished.save(&channel) {
                                    warn!("Failed to save Auction data: {}", e);
                                }

                                #[cfg(feature = "csv")]
                                if let Some(path) = &opt_csv {
                                    if let Err(e) = finished.save_csv(path) {
                                        warn!("Failed to write CSV: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    time += INTERVAL;
                    sleep(time.saturating_duration_since(Instant::now()));
                }

                running.store(false, SeqCst);
                block_on(cli.quit());
            })?
        };

        let bot_exit = self.main_loop(runner, client, run_thread).await;

        match auction_thread.join() {
            Err(e) => Err(BotExit::ThreadPanic(e)),
            Ok(()) => Ok(bot_exit),
        }
    }

    async fn main_loop(
        &mut self,
        mut runner: AsyncRunner,
        client: Client,
        running: Arc<AtomicBool>,
    ) -> BotExit {
        self.client = Some(client);

        let bot_exit = loop {
            match runner.next_message().await {
                Ok(Status::Message(msg)) => self.handle_message(msg).await,
                Ok(Status::Quit) => break BotExit::BotExited,
                Ok(Status::Eof) => break BotExit::ConnectionClosed,
                Err(error) => break error.into(),
            }

            if !running.load(SeqCst) {
                break BotExit::ThreadStopped;
            }
        };

        running.store(false, SeqCst);
        self.client = None;
        self.stopped = Some(Instant::now());
        bot_exit
    }

    async fn handle_command(
        &mut self,
        msg: &Privmsg<'_>,
        line: &str,
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

                Some(Reply(match auction.last_bid() {
                    None => format!(
                        "The {} still has {} remaining. The minimum bid \
                        is {}, but there have not been any bids yet.",
                        auction.describe(),
                        time,
                        usd!(auction.min_bid),
                    ),
                    Some(Bid { amount, bidder, .. }) => format!(
                        "The {} still has {} remaining. The leader is \
                        currently {}, who bids {}.",
                        auction.describe(),
                        time,
                        bidder,
                        usd!(amount),
                    ),
                }))
            }
            ["auction", subcom, args @ ..] if usr_op => match *subcom {
                "prize" => {
                    let mut lock = self.auction.lock();
                    let auction = lock.as_mut()?;

                    // let prize = auction.prize.take();
                    auction.prize = to_end_unquoted(line, args)
                        .map(|s| s.to_owned());

                    Some(Reply(match &auction.prize {
                        Some(s) => format!("The current Auction is for {}.", s),
                        None => format!("The Auction prize has been unset."),
                    }))
                }
                "start" => {
                    let mut lock = self.auction.lock();

                    if lock.is_some() {
                        Some(Reply(format!(
                            "An Auction is already running; Invoke '{}auction \
                            stop' to cancel it.",
                            self.config.prefix(),
                        )))
                    } else {
                        let channel = msg.channel().trim_start_matches('#');
                        let mut dur = self.config.duration(channel);
                        let mut hlm = self.config.helmet(channel);
                        let mut max = self.config.max_raise(channel);
                        let mut min = self.config.min_bid(channel);
                        let mut vrb = self.config.verb(channel);
                        let mut tok = args.iter();
                        let mut prz = None;

                        while let Some(flag) = tok.next() {
                            match *flag {
                                "-d" | "-t" | "--time"
                                => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        dur = Duration::from_secs(vl);
                                    }
                                }
                                "-h" | "--helm" | "--helmet"
                                => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        hlm = Duration::from_secs(vl);
                                    }
                                }
                                "-r" | "--raise" | "--limit"
                                => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        max = vl;
                                    }
                                }
                                "-m" | "--min"
                                => if let Some(val) = tok.next() {
                                    if let Ok(vl) = val.parse() {
                                        min = vl;
                                    }
                                }
                                "-v" | "--verb"
                                => if let Some(val) = tok.next() {
                                    vrb = val;
                                }
                                "--prize" => {
                                    prz = tok.next();
                                }
                                _ => {}
                            }
                        }

                        info!("Auction in #{} started by {}.", channel, author);
                        let new: &mut Auction = lock.insert(Auction::new(
                            dur, hlm, max, min, prz.map(|s| String::from(
                                unquote(s),
                            )),
                        ));

                        Some(Message(new.explain(self.config.prefix(), &vrb)))
                    }
                }
                "stop" => Some(Reply(match self.auction.lock().take() {
                    Some(..) => String::from("Auction stopped."),
                    None => String::from("No Auction is currently running."),
                })),
                _ => None,
            }
            ["bid", value, ..] => match unquote(value)
                .trim_start_matches('$')
                .parse::<usize>()
            {
                Ok(bid) => Some(match self.auction.lock()
                    .as_mut()?
                    .bid(&author, bid)
                {
                    BidResult::Ok { first } => Message(format!(
                        "{} BID: @{} has bid {}.",
                        if first { "FIRST" } else { "NEW" },
                        author,
                        usd!(bid),
                    )),
                    BidResult::RepeatBidder(bid) => Reply(format!(
                        "You are already the top bidder at {}.",
                        usd!(bid),
                    )),
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
                Err(..) if self.auction.lock().is_some() => Some(Reply(format!(
                    "A bid must be a positive whole number of {}.",
                    usd!(),
                ))),
                _ => None,
            }
            ["config", ..] if usr_op => {
                let channel = msg.channel().trim_start_matches('#');

                Some(Reply(format!(
                    "Auction length is {dur} seconds. \
                    Helmet value is {hlm} seconds. \
                    Minimum bid is {min}. \
                    Maximum raise is {max}.",
                    dur = self.config.duration(channel).as_secs(),
                    hlm = self.config.helmet(channel).as_secs(),
                    max = usd!(self.config.max_raise(channel)),
                    min = usd!(self.config.min_bid(channel)),
                )))
            }
            #[cfg(debug_assertions)]
            ["die", ..] if usr_op => {
                info!("Bot killed by {}.", author);
                self.client.as_ref()?.clone().quit().await;
                None
            }
            // #[cfg(debug_assertions)]
            ["echo", arg, ..] if usr_op => Some(Message(format!(
                "{} said: {:?}",
                author, substring_to_end(line, arg).unwrap_or(arg),
            ))),
            ["reload", ..] if usr_op => Some(Reply(match self.config.reload() {
                Ok(..) => String::from("Configuration reloaded."),
                Err(_) => String::from("Failed to reload Config."),
            })),
            _ => None,
        }
    }

    async fn handle_message(&mut self, message: Commands<'_>) {
        use Commands::*;

        match message {
            Privmsg(msg) if !self.should_ignore(&msg)
            => if let Some((line, words)) = self.find_command(msg.data()) {
                chat!("({}) {}: {:?}", msg.channel(), msg.name(), msg.data());

                if let Some(reply) = self.handle_command(&msg, line, &words).await {
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

    pub fn find_command<'s>(&self, text: &'s str) -> Option<(&'s str, Vec<&'s str>)> {
        match text.strip_prefix(self.config.prefix()) {
            Some(line) => if self.config.parse_commands() {
                Some(split_cmd(line))
            } else {
                Some((line, line.split_whitespace().collect()))
            }
            None => None,
        }
    }
}
