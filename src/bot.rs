mod auction;

use auction::*;
use std::time::{Duration, Instant};


struct Timer {
    next: Instant,
}

impl Timer {
    const TICK: Duration = Duration::from_secs(1);

    fn new() -> Self { Self { next: Instant::now() + Self::TICK } }

    fn tick(&mut self) -> bool {
        if Instant::now() <= self.next {
            self.next += Self::TICK;
            true
        } else {
            false
        }
    }
}


pub struct Bot {
    channel: String,
    auction: Option<Auction>,
}

impl Bot {
    pub fn new(channel: String) -> Self {
        Self {
            channel,
            auction: None,
        }
    }

    pub fn emit(&self, content: impl std::fmt::Display) {
        println!("BOT -> {}: {}", self.channel, content);
    }

    pub fn run(&mut self) {
        let mut timer = Timer::new();

        //  TODO: Set up IRC connection.

        while crate::running() {
            //  TODO: Handle all pending IRC messages.

            while timer.tick() {
                self.auction_check();
            }
        }
    }
}

impl Bot {
    fn auction_check(&mut self) {
        if let Some(auction) = &self.auction {
            match auction.remaining() {
                Some(time) => match time.as_secs() {
                    // t @ 1 => {
                    //     self.emit(format!("Auction: {} second remains.", t));
                    // }
                    1 => self.emit("Auction: 1 second remains."),

                    // t @ (2 | 3 | 4 | 5 | 10 | 15 | 30 | 60 // <=1m
                    t @ (2..=5 | 10 | 15 | 30 | 60 // <=1m
                    | 120 | 300 | 600 | 900 | 1800 | 3600 // <=1h
                    | 7200 | 10800) => {
                        self.emit(format!("Auction: {} seconds remain.", t));
                    }

                    _ => {} // NOP
                }
                None => {
                    if let Some(Bid { amount, bidder }) = auction.get_bid() {
                        self.emit(format!(
                            "The Auction has been won by @{}, with a bid of {}.",
                            bidder, currency!(amount),
                        ));
                    } else {
                        self.emit("The Auction has ended with no bids.");
                    }

                    self.auction.take();
                }
            }
        }
    }
}
