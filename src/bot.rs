mod auction;

use auction::*;


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

    pub fn run(&mut self) {}
}
