#![cfg(feature = "csv")]

use std::{fs::File, path::Path};
#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use csv::{QuoteStyle, ReaderBuilder, Terminator, WriterBuilder};
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
    pub winning_bid: Option<usize>,
    pub winner: Option<String>,
    pub prize: Option<String>,
    pub note: Option<String>,
}

impl AuctionRecord {
    //  NOTE: This method exists to ensure at compile time that all fields are
    //      present, regardless of what Features are enabled. No Feature checks
    //      should be made here.
    fn _drop(self) {
        self.opened;
        self.closed;
        self.duration_seconds;
        self.winning_bid;
        self.winner;
        self.prize;
        self.note;
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
            winning_bid,
            winner,
            prize: auction.prize.clone(),
            note: None,
        }
    }
}


impl AuctionFinished {
    fn csv_reader(headers: bool) -> ReaderBuilder {
        let mut rb = ReaderBuilder::new();

        rb.has_headers(headers);
        rb.terminator(Terminator::CRLF);

        rb
    }

    fn csv_writer(headers: bool) -> WriterBuilder {
        let mut wb = WriterBuilder::new();

        wb.has_headers(headers);
        wb.quote_style(QuoteStyle::NonNumeric);
        wb.terminator(Terminator::CRLF);

        wb
    }

    pub fn save_csv(&self, path: &Path) -> std::io::Result<()> {
        let mut csv = if path.exists() {
            if cfg!(feature = "csv_validate") {
                let mut read = Self::csv_reader(true).from_path(&path)?;
                let mut iter = read.deserialize::<AuctionRecord>();

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

    fn to_record(&self) -> AuctionRecord { self.into() }
}
