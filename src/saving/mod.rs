mod csv_record;

use std::{fmt::Display, fs::File, io::Write};
#[cfg(feature = "chrono")]
use chrono::{Datelike, DateTime, Duration, SubsecRound, Timelike, Utc};
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
    const FILE_EXT: &'static str = "toml";

    fn file_name(&self, channel: &str) -> String {
        match &self.prize {
            Some(prize) => format!(
                "{stem}-{prize}.{ext}",
                stem = self.file_stem(channel),
                prize = prize.to_snake_case(),
                ext = Self::FILE_EXT,
            ),
            None => format!(
                "{stem}.{ext}",
                stem = self.file_stem(channel),
                ext = Self::FILE_EXT,
            ),
        }
    }

    fn file_stem(&self, channel: &str) -> String {
        format!(
            "auction-{}-{}",
            //  Hash trimming is redundant for now, but just to be future safe.
            channel.trim_start_matches('#'),
            self.timestamp(),
        )
    }

    #[cfg(feature = "chrono")]
    fn timestamp(&self) -> impl Display {
        format!(
            //  yyyymmdd-hhmmss
            "{:0>4}{:0>2}{:0>2}\
            -{:0>2}{:0>2}{:0>2}",
            // //  yyyymmddThhmmssZ
            // "{:0>4}{:0>2}{:0>2}\
            // T{:0>2}{:0>2}{:0>2}Z",
            self.opened.year(),
            self.opened.month(),
            self.opened.day(),
            self.opened.hour(),
            self.opened.minute(),
            self.opened.second(),
        )

        // self.opened.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    #[cfg(not(feature = "chrono"))]
    fn timestamp(&self) -> impl Display {
        std::time::SystemTime::UNIX_EPOCH.elapsed()
            .unwrap_or_default()
            .as_secs()
    }
}

impl AuctionFinished {
    pub fn save(&self, channel: &str) -> std::io::Result<()> {
        info!("Saving auction data...");

        match ProjectDirs::from("", "", env!("CARGO_PKG_NAME")) {
            Some(dirs) => match toml::to_vec(self) {
                Ok(data) => {
                    let mut path = dirs.data_dir().to_owned();
                    std::fs::create_dir_all(&path)?;
                    path.push(self.file_name(channel));

                    let mut file = File::create(&path)?;
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

#[cfg(feature = "csv")]
impl AuctionFinished {
    fn csv_reader(headers: bool) -> csv::ReaderBuilder {
        let mut rb = csv::ReaderBuilder::new();

        rb.has_headers(headers);
        rb.terminator(csv::Terminator::CRLF);

        rb
    }

    fn csv_writer(headers: bool) -> csv::WriterBuilder {
        let mut wb = csv::WriterBuilder::new();

        wb.has_headers(headers);
        wb.quote_style(csv::QuoteStyle::NonNumeric);
        wb.terminator(csv::Terminator::CRLF);

        wb
    }

    pub fn save_csv(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut csv = if path.exists() {
            if cfg!(feature = "csv_validate") {
                let mut read = Self::csv_reader(true).from_path(&path)?;
                let mut iter = read.deserialize::<csv_record::AuctionRecord>();

                if let Some(record) = iter.next() {
                    record?;
                }
            }

            Self::csv_writer(false)
                .from_writer(File::options()
                    .append(true)
                    .open(&path)?)
        } else {
            Self::csv_writer(true).from_path(&path)?
        };

        csv.serialize(self.to_record())?;
        csv.flush()?;

        info!("Saved record to spreadsheet: {}", path.display());

        Ok(())
    }

    fn to_record(&self) -> csv_record::AuctionRecord { self.into() }
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
