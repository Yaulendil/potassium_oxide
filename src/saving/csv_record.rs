#![cfg(feature = "csv")]

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use super::{AuctionFinished, Winner};


#[derive(Debug, Deserialize, Serialize)]
pub struct AuctionRecord {
    #[cfg(feature = "chrono")]
    pub opened: DateTime<Utc>,
    #[cfg(not(feature = "chrono"))]
    pub opened: (),
    pub time: u64,
    pub prize: Option<String>,
    pub winner: Option<String>,
    pub winning_bid: Option<usize>,
}

impl AuctionRecord {
    //  NOTE: This method exists to ensure at compile time that all fields are
    //      present, regardless of what Features are enabled. No Feature checks
    //      should be made here.
    fn _drop(self) {
        self.opened;
        self.time;
        self.prize;
        self.winner;
        self.winning_bid;
    }
}

impl From<&AuctionFinished> for AuctionRecord {
    fn from(auction: &AuctionFinished) -> Self {
        let winner: Option<String>;
        let winning_bid: Option<usize>;

        match &auction.winner {
            Some(Winner { name, amount, .. }) => {
                winner = Some(name.clone());
                winning_bid = Some(*amount);
            }
            None => {
                winner = None;
                winning_bid = None;
            }
        }

        Self {
            #[cfg(feature = "chrono")]
            opened: auction.opened,
            #[cfg(not(feature = "chrono"))]
            opened: (),

            time: auction.duration,
            prize: auction.prize.clone(),
            winner,
            winning_bid,
        }
    }
}
