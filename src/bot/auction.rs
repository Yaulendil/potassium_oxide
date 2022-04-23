use std::time::{Duration, Instant};


pub struct Bid {
    pub amount: usize,
    pub bidder: String,
}


pub enum BidResult {
    Ok { first: bool },
    RepeatBidder,
    AboveMaximum(usize),
    BelowMinimum(usize),
    DoesNotRaise(usize),
}


pub struct Auction {
    current_bid: Option<Bid>,

    helmet: Duration,
    max_raise: usize,
    min_bid: usize,

    time_begin: Instant,
    time_close: Instant,
}

impl Auction {
    pub fn new(
        duration: Duration,
        helmet: Duration,
        max_raise: usize,
        min_bid: usize,
    ) -> Self {
        let now = Instant::now();

        Self {
            current_bid: None,
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
        let first: bool = if let Some(Bid {
            amount: bid_current,
            bidder: ref name_current,
        }) = self.current_bid {
            if name_new.as_ref().eq_ignore_ascii_case(name_current) {
                return BidResult::RepeatBidder;
            }

            if bid_new <= bid_current {
                return BidResult::DoesNotRaise(bid_current);
            }

            if self.max_raise < bid_new.saturating_sub(bid_current) {
                return BidResult::AboveMaximum(self.max_raise);
            }

            false
        } else {
            true
        };

        if bid_new < self.min_bid {
            BidResult::BelowMinimum(self.min_bid)
        } else {
            self.current_bid.replace(Bid {
                amount: bid_new,
                bidder: name_new.as_ref().to_string(),
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

    pub fn get_bid(&self) -> Option<&Bid> {
        self.current_bid.as_ref()
    }

    pub fn get_minimum(&self) -> usize { self.min_bid }

    pub fn remaining(&self) -> Option<Duration> {
        self.time_close.checked_duration_since(Instant::now())
            .map(|d| Duration::new(d.as_secs(), 0))
    }
}

impl Auction {
    pub fn explain(&self, prefix: &str, verb: &str) -> String {
        format!(
            "ATTENTION: The Auction will run for {time}. Submit a bid by \
            posting '{prefix}bid <amount>'. Focus on this chat, NOT any 'live' \
            video, since there may be a delay. You cannot raise by more than \
            {max_raise}. I will confirm bids in chat. At the end, I will do a \
            final 5–1 countdown, after which the Auction will be over. The \
            person with the highest bid at that time will be declared the \
            winner, and they will have to {verb} that amount in order to claim \
            their prize. Bidding starts at {min_bid}.",
            max_raise=usd!(self.max_raise),
            min_bid=usd!(self.min_bid),
            prefix=prefix,
            time=humantime::format_duration(
                self.time_close.saturating_duration_since(self.time_begin)
            ),
            verb=verb,
        )
    }
}
