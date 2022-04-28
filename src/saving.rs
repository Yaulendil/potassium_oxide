use std::io::Write;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Duration, SecondsFormat, SubsecRound, Utc};
use directories::ProjectDirs;
use heck::SnakeCase;
use crate::bot::auction::{Auction, Bid, Winner};


#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "summaries_pascal", serde(rename_all = "PascalCase"))]
pub struct AuctionFinished {
    pub prize: Option<String>,
    pub minimum_bid: usize,
    pub raise_limit: usize,
    pub duration: u64,
    pub helmet: u64,

    #[cfg(feature = "chrono")]
    pub opened: DateTime<Utc>,
    #[cfg(feature = "chrono")]
    pub closed: DateTime<Utc>,

    #[serde(rename = "WINNER")]
    pub winner: Option<Winner>,
    #[serde(rename = "BID", skip_serializing_if = "Vec::is_empty")]
    pub bids: Vec<Bid>,
}

impl AuctionFinished {
    pub fn save(&self, channel: &str) -> std::io::Result<()> {
        info!("Saving auction data...");

        match ProjectDirs::from("", "", env!("CARGO_PKG_NAME")) {
            Some(dirs) => match toml::to_vec(self) {
                Ok(data) => {
                    let mut path = dirs.data_dir().to_owned();
                    std::fs::create_dir_all(&path)?;

                    #[cfg(feature = "chrono")]
                    let ts = self.opened.to_rfc3339_opts(
                        SecondsFormat::Secs,
                        true,
                    );
                    #[cfg(not(feature = "chrono"))]
                    let ts = std::time::SystemTime::UNIX_EPOCH.elapsed()
                        .unwrap_or_default().as_secs();

                    let prize = match &self.prize {
                        Some(prize) => format!("-{}", prize.to_snake_case()),
                        None => String::new(),
                    };

                    path.push(format!(
                        "auction-{}-{}{}.toml",
                        channel, ts, prize,
                    ));
                    let mut file = std::fs::File::create(&path)?;
                    file.write_all(&data)?;

                    info!("Saved to file: {}", path.display());
                }
                Err(e) => warn!("Failed to serialize data: {}", e),
            }
            None => warn!("Failed to find data directory."),
        }

        Ok(())
    }
}

impl From<Auction> for AuctionFinished {
    fn from(auction: Auction) -> Self {
        let winner = auction.winner();
        #[allow(unused_variables)]
        let Auction {
            bids, prize,
            duration, helmet,
            max_raise, min_bid,
            time_begin, time_close,
        } = auction;

        #[cfg(feature = "chrono")]
        let (opened, closed) = {
            let now = std::time::Instant::now();
            let now_utc: DateTime<Utc> = Utc::now();

            let since_begin = Duration::from_std(now - time_begin)
                .unwrap_or_else(|_| Duration::zero());
            let since_close = Duration::from_std(now - time_close)
                .unwrap_or_else(|_| Duration::zero());

            let opened = (now_utc - since_begin).round_subsecs(0);
            let closed = (now_utc - since_close).round_subsecs(0);

            (opened, closed)
        };

        Self {
            prize,
            minimum_bid: min_bid,
            raise_limit: max_raise,
            duration: duration.as_secs(),
            helmet: helmet.as_secs(),

            #[cfg(feature = "chrono")]
            opened,
            #[cfg(feature = "chrono")]
            closed,

            winner,
            bids,
        }
    }
}
