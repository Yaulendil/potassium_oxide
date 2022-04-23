use std::time::{Duration, Instant};
#[cfg(feature = "chrono")]
use chrono::SubsecRound;
use crate::AuctionFinished;


#[derive(Deserialize, Serialize)]
// #[serde(rename_all = "PascalCase")]
pub struct Bid {
    pub amount: usize,
    pub bidder: String,
    #[cfg(feature = "chrono")]
    pub time: chrono::DateTime<chrono::Utc>,
}


pub enum BidResult {
    Ok { first: bool },
    RepeatBidder,
    AboveMaximum(usize),
    BelowMinimum(usize),
    DoesNotRaise(usize),
}


pub struct Auction {
    pub bids: Vec<Bid>,
    pub prize: Option<String>,

    pub duration: Duration,
    pub helmet: Duration,
    pub max_raise: usize,
    pub min_bid: usize,

    pub time_begin: Instant,
    pub time_close: Instant,
}

impl Auction {
    pub fn new(
        duration: Duration,
        helmet: Duration,
        max_raise: usize,
        min_bid: usize,
        prize: Option<String>,
    ) -> Self {
        let now = Instant::now();

        Self {
            bids: Vec::new(),
            prize,
            duration,
            helmet,
            max_raise,
            min_bid,
            time_begin: now,
            time_close: now + duration,
        }
    }

    pub fn add_time(&mut self, time: Duration) {
        self.time_close += time;
    }

    pub fn bid(
        &mut self,
        name_new: impl AsRef<str>,
        bid_new: usize,
    ) -> BidResult {
        let name_new = name_new.as_ref();

        let first: bool = if let Some(Bid {
            amount: bid_current,
            bidder: name_current,
            ..
        }) = self.last_bid() {
            if name_new.eq_ignore_ascii_case(name_current) {
                info!("Bid by {} refused (repeat).", name_new);
                return BidResult::RepeatBidder;
            }

            if bid_new <= *bid_current {
                info!("Bid by {} refused (too low).", name_new);
                return BidResult::DoesNotRaise(*bid_current);
            }

            if self.max_raise < bid_new.saturating_sub(*bid_current) {
                info!("Bid by {} refused (too high).", name_new);
                return BidResult::AboveMaximum(self.max_raise);
            }

            false
        } else {
            true
        };

        if bid_new < self.min_bid {
            info!("Bid by {} refused (too low).", name_new);
            BidResult::BelowMinimum(self.min_bid)
        } else {
            info!("New bid: {} by {}.", usd!(bid_new), name_new);
            self.bids.push(Bid {
                amount: bid_new,
                bidder: name_new.to_string(),
                #[cfg(feature = "chrono")]
                time: chrono::Utc::now().round_subsecs(3),
            });

            self.deflect_sniper();
            BidResult::Ok { first }
        }
    }

    fn deflect_sniper(&mut self) {
        let now = Instant::now();

        if (self.time_close - self.helmet) < now {
            self.add_time(self.helmet);
        }
    }

    pub fn last_bid(&self) -> Option<&Bid> {
        self.bids.last()
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.time_close.checked_duration_since(Instant::now())
            .map(|d| Duration::new(d.as_secs(), 0))
    }

    pub fn finish(self) -> AuctionFinished { self.into() }
}

impl Auction {
    pub fn describe(&self) -> String {
        format!("Auction{}", self.for_prize())
    }

    pub fn explain(&self, prefix: &str, verb: &str) -> String {
        format!(
            "ATTENTION: An {auction} will now run for {time}. Submit a bid by \
            posting '{prefix}bid <amount>'. Focus on this chat, NOT any 'live' \
            video, since there may be a delay. You cannot raise by more than \
            {max_raise}. I will confirm bids in chat. At the end, I will do a \
            final 5–1 countdown, after which the Auction will be over. The \
            person with the highest bid at that time will be declared the \
            winner, and they will have to {verb} that amount in order to claim \
            their prize. Bidding starts at {min_bid}.",
            auction = self.describe(),
            max_raise = usd!(self.max_raise),
            min_bid = usd!(self.min_bid),
            prefix = prefix,
            time = humantime::format_duration(
                self.time_close.saturating_duration_since(self.time_begin)
            ),
            verb = verb,
        )
    }

    pub fn for_prize(&self) -> String {
        self.prize.as_ref()
            .map(|s| format!(" for {s}"))
            .unwrap_or_default()
    }
}
