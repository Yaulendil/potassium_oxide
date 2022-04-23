use std::io::Write;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Duration, SecondsFormat, SubsecRound, Utc};
use directories::ProjectDirs;
use crate::bot::auction::{Auction, Bid};


#[derive(Deserialize, Serialize)]
// #[serde(rename_all = "PascalCase")]
pub struct AuctionFinished {
    pub minimum_bid: usize,
    pub raise_limit: usize,
    pub duration: u64,
    pub helmet: u64,

    #[cfg(feature = "chrono")]
    pub opened: DateTime<Utc>,
    #[cfg(feature = "chrono")]
    pub closed: DateTime<Utc>,

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

                    path.push(format!("auction-{}-{}.toml", channel, ts));
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
        #[cfg(feature = "chrono")]
        let (opened, closed) = {
            let now = std::time::Instant::now();
            let now_utc = Utc::now();

            let since_begin = Duration::from_std(now - auction.time_begin)
                .unwrap_or_else(|_| Duration::zero());
            let since_close = Duration::from_std(now - auction.time_close)
                .unwrap_or_else(|_| Duration::zero());

            let opened = (now_utc - since_begin).round_subsecs(0);
            let closed = (now_utc - since_close).round_subsecs(0);

            (opened, closed)
        };

        Self {
            bids: auction.bids,

            minimum_bid: auction.min_bid,
            raise_limit: auction.max_raise,
            duration: auction.duration.as_secs(),
            helmet: auction.helmet.as_secs(),

            #[cfg(feature = "chrono")]
            opened,
            #[cfg(feature = "chrono")]
            closed,
        }
    }
}
