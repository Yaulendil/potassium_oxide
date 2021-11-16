mod auction;
mod client;

use auction::*;
use client::Client;
use crate::config::Config;
use parking_lot::Mutex;
use smol::block_on;
use std::{sync::Arc, thread::{sleep, spawn}, time::{Duration, Instant}};
use twitchchat::{connector, messages, runner::AsyncRunner, Status, UserConfig};


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
                let out = match auction.get_bid() {
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

    pub async fn send(&self, msg: impl AsRef<str>) {
        if let Some(mut client) = self.client.clone() {
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

            println!("Connected.");
            // println!("Connected. Identity: {:#?}", runner.identity);

            let client = Client::new(self.channel.clone(), &mut runner).await;

            let auction_loop = {
                let mut client = client.clone();
                let arc = self.auction.clone();

                spawn(move || {
                    const INC: Duration = Duration::from_secs(1);

                    let mut time = Instant::now();

                    while crate::running() {
                        if let Some(mut lock) = arc.try_lock_until(time) {
                            if let Some(text) = auction_check(&mut lock) {
                                block_on(client.send(text)).unwrap();
                            }
                        }

                        time += INC;
                        sleep(time.saturating_duration_since(Instant::now()));
                    }

                    block_on(client.quit());
                })
            };

            self.client = Some(client);

            let res = self.main_loop(runner).await;
            auction_loop.join().expect("Failed to rejoin Auction thread.");
            println!();
            res
        };

        block_on(future)
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
                        if lock.is_some() {
                            self.send("An Auction is already running; Invoke \
                            '+auction stop' to cancel it.").await;
                            return;
                        }

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

                        let new = lock.insert(Auction::new(
                            &self.config,
                            Duration::from_secs(sec),
                            min,
                        ));

                        self.send(new.explain(&self.config.bot.prefix)).await;
                    }
                    "stop" => match self.auction.lock().take() {
                        Some(..) => self.send("Auction stopped.").await,
                        None => self.send("No Auction is currently running.").await,
                    },
                    _ => {}
                }
            }
            ["bid", arg, ..] => match substring_to_end(msg.data(), arg)
                .unwrap_or(arg).trim_start_matches('$').parse::<usize>()
            {
                Ok(bid) => if let Some(auction) = self.auction.lock().as_mut() {
                    match auction.bid(&author, bid) {
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
