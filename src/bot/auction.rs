use std::time::{Duration, SystemTime, SystemTimeError};


/// Helmets protect against snipers.
const HELMET: Duration = Duration::from_secs(15);
type Money = usize;


pub struct Bid {
    bidder: String,
    amount: Money,
}


pub enum BidResult {
    Ok,
    RepeatBidder,
    AboveMaximum(Money),
    BelowMinimum(Money),
    DoesNotRaise(Money),
}


pub struct Auction {
    current_bid: Option<Bid>,

    max_raise: Money,
    min_bid: Money,

    time_begin: SystemTime,
    time_close: SystemTime,
}

impl Auction {
    pub fn new(duration: Duration, max_raise: Money, min_bid: Money) -> Self {
        let now = SystemTime::now();

        Self {
            current_bid: None,
            max_raise,
            min_bid,
            time_begin: now,
            time_close: now + duration,
        }
    }

    pub fn bid(
        &mut self,
        name_new: impl AsRef<str>,
        bid_new: Money,
    ) -> BidResult {
        if let Some(Bid {
            bidder: ref name_current,
            amount: bid_current,
        }) = self.current_bid {
            if name_new.as_ref() == name_current.as_str() {
                return BidResult::RepeatBidder;
            }

            if bid_new <= bid_current {
                return BidResult::DoesNotRaise(bid_current);
            }

            if self.max_raise < bid_new.saturating_sub(bid_current) {
                return BidResult::AboveMaximum(self.max_raise);
            }
        }

        if bid_new < self.min_bid {
            return BidResult::BelowMinimum(self.min_bid);
        }

        self.current_bid.replace(Bid {
            bidder: name_new.as_ref().to_string(),
            amount: bid_new,
        });
        self.deflect_sniper();
        BidResult::Ok
    }

    fn deflect_sniper(&mut self) {
        let now = SystemTime::now();

        match self.time_close.duration_since(now) {
            Ok(dur) if dur > HELMET => {}
            _ => { self.time_close = now + HELMET; }
        }
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.time_close.duration_since(SystemTime::now()).ok()
    }
}
