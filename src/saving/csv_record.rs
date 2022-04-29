#![cfg(feature = "csv")]

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use super::{AuctionFinished, Winner};


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuctionRecord {
    #[cfg(feature = "chrono")]
    pub opened: DateTime<Utc>,
    #[cfg(not(feature = "chrono"))]
    pub opened: (),

    #[cfg(feature = "chrono")]
    pub closed: DateTime<Utc>,
    #[cfg(not(feature = "chrono"))]
    pub closed: (),

    pub duration_seconds: u64,
    pub winner: Option<String>,
    pub winning_bid: Option<usize>,
    pub prize: Option<String>,
}

impl AuctionRecord {
    //  NOTE: This method exists to ensure at compile time that all fields are
    //      present, regardless of what Features are enabled. No Feature checks
    //      should be made here.
    fn _drop(self) {
        self.opened;
        self.closed;
        self.duration_seconds;
        self.winner;
        self.winning_bid;
        self.prize;
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

            #[cfg(feature = "chrono")]
            closed: auction.closed,
            #[cfg(not(feature = "chrono"))]
            closed: (),

            duration_seconds: auction.duration,
            winner,
            winning_bid,
            prize: auction.prize.clone(),
        }
    }
}
