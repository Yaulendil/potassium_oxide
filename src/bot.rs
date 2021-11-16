mod auction;
mod client;

use auction::*;
use client::Client;
use crate::config::Config;
use parking_lot::Mutex;
use std::{sync::Arc, time::{Duration, Instant}};
use twitchchat::{connector, messages, runner::AsyncRunner, Status, UserConfig};


fn substring_to_end<'a, 'b>(main: &'a str, sub: &'b str) -> Option<&'a str> {
    let valid = main.as_bytes().as_ptr_range();

    if !sub.is_empty() && valid.contains(&sub.as_ptr()) {
        let idx = unsafe { sub.as_ptr().offset_from(valid.start) } as usize;

        Some(&main[idx..])
    } else {
        None
    }
}


pub enum BotExit {
    BotClosed,
    BotExited,
    ClientErr,
    ConfigErr,
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

    pub fn emit(&self, content: impl std::fmt::Display) {
        println!("BOT -> [#{}] {}", self.channel, content);
    }

    pub async fn send(&self, msg: impl AsRef<str>) {
        if let Some(mut client) = self.client.clone() {
            self.emit(msg.as_ref());
            client.send(msg).await.unwrap();
        }
    }

    pub fn run(&mut self) -> Result<BotExit, BotExit> {
        let uconf = UserConfig::builder()
            .name(&self.config.auth.username)
            .token(&self.config.auth.oauth)
            .enable_all_capabilities()
            .build()
            .expect("o no");

        let future = async move {
            println!("Connecting...");
            let mut runner = AsyncRunner::connect(
                connector::smol::Connector::twitch().unwrap(),
                &uconf,
            ).await.unwrap();

            println!("Connected. Identity: {:#?}", runner.identity);

            let client = Client::new(self.channel.clone(), &mut runner).await;
            let timer = smol::spawn({
                let mut client = client.clone();

                async move {
                    smol::Timer::after(Duration::from_secs(10)).await;

                    client.send("qwert").await.unwrap();
                    smol::Timer::after(Duration::from_secs(1)).await;
                    client.quit().await;
                }
            });

            self.client = Some(client);
            self.send("asdf").await;

            let res = self.main_loop(runner).await;
            timer.await;
            res
        };

        smol::block_on(future)
    }

    async fn main_loop(&mut self, mut runner: AsyncRunner) -> Result<BotExit, BotExit> {
        loop {
            match runner.next_message().await.unwrap() {
                Status::Message(msg) => self.handle_message(msg).await,
                Status::Quit => break Ok(BotExit::BotExited),
                Status::Eof => break Ok(BotExit::BotClosed),
            }
        }
    }

    async fn handle_command(&mut self, msg: &messages::Privmsg<'_>, words: &[&str]) {
        let author = msg.display_name().unwrap_or_else(|| msg.name());

        match words {
            ["auction", subcom, args @ ..]
            => if self.config.bot.admins.contains(&msg.name().to_owned())
                || msg.is_broadcaster()
                || msg.is_moderator()
            {
                match *subcom {
                    "start" => {
                        let mut lock = self.auction.lock();
                        if lock.is_some() { return; }

                        let mut itr = args.iter();
                        let mut min = self.config.bot.default_minimum;
                        let mut sec = self.config.bot.default_duration;

                        while let Some(flag) = itr.next() {
                            match *flag {
                                "-m" => if let Some(val) = itr.next() {
                                    if let Ok(v) = val.parse() {
                                        min = v;
                                    }
                                }
                                "-t" => if let Some(val) = itr.next() {
                                    if let Ok(v) = val.parse() {
                                        sec = v;
                                    }
                                }
                                _ => {}
                            }
                        }

                        *lock = Some(Auction::new(
                            &self.config,
                            Duration::from_secs(sec),
                            min,
                        ));
                    }
                    "stop" => {
                        let mut lock = self.auction.lock();
                        if lock.is_none() { return; }

                        *lock = None;
                    }
                    _ => {}
                }
            }
            ["bid", arg, ..] => match substring_to_end(msg.data(), arg)
                .unwrap_or(arg).trim_start_matches('$').parse::<usize>()
            {
                Ok(bid) => if let Some(auction) = self.auction.lock().as_mut() {
                    match auction.bid(msg.name(), bid) {
                        BidResult::Ok => self.send(&format!(
                            "@{} has bid {}.",
                            author, usd!(bid),
                        )).await,
                        BidResult::RepeatBidder => self.send(&format!(
                            "@{}: You are already the top bidder.",
                            author,
                        )).await,
                        BidResult::AboveMaximum(max) => self.send(&format!(
                            "@{}: You can only raise by a maximum of {}.",
                            author, usd!(max),
                        )).await,
                        BidResult::BelowMinimum(min) => self.send(&format!(
                            "@{}: The minimum bid is {}.",
                            author, usd!(min),
                        )).await,
                        BidResult::DoesNotRaise(cur) => self.send(&format!(
                            "@{}: The current bid is {}.",
                            author, usd!(cur),
                        )).await,
                    }
                }
                Err(..) => {
                    self.send("A bid must be a whole number of USD.").await;
                }
            }
            ["echo", arg, ..] => self.send(&format!(
                "{} said: {:?}",
                author, substring_to_end(msg.data(), arg).unwrap_or(arg),
            )).await,
            _ => {}
        }
    }

    async fn handle_message(&mut self, message: messages::Commands<'_>) {
        use messages::Commands::*;

        match message {
            Privmsg(msg) if !self.config.bot.blacklist.contains(&msg.name().to_owned())
            => if let Some(line) = msg.data().strip_prefix(&self.config.bot.prefix) {
                println!("[{}] {}: {}", msg.channel(), msg.name(), msg.data());

                let words: Vec<&str> = line.split_whitespace().collect();

                self.handle_command(&msg, &words).await;
            },

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
}

impl<'b> Bot<'b> {
    async fn auction_check(&self) {
        let mut lock = self.auction.lock();

        if let Some(auction) = lock.as_mut() {
            match auction.remaining() {
                Some(time) => match time.as_secs() {
                    t @ 1..=5 => {
                        self.send(format!("Auction: {}...", t)).await;
                    }

                    // t @ 1 => {
                    //     self.emit(format!("Auction: {} second remains.", t));
                    // }
                    1 => self.send("Auction: 1 second remains.").await,

                    // t @ (2 | 3 | 4 | 5 | 10 | 15 | 30 | 60 // <=1m
                    t @ (2..=5 | 10 | 15 | 30 | 60 // <=1m
                    | 120 | 300 | 600 | 900 | 1800 | 3600 // <=1h
                    | 7200 | 10800) => {
                        self.send(format!("Auction: {} seconds remain.", t)).await;
                    }

                    _ => {} // NOP
                }
                None => {
                    if let Some(Bid { amount, bidder }) = auction.get_bid() {
                        self.send(format!(
                            "The Auction has been won by @{}, with a bid of {}.",
                            bidder, usd!(amount),
                        )).await;
                    } else {
                        self.send("The Auction has ended with no bids.").await;
                    }

                    lock.take();
                }
            }
        }
    }
}
