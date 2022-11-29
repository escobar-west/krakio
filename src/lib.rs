use serde::Deserialize;
use std::collections::BTreeMap;
use termion::event::Key;

pub enum Event {
    Input(Key),
    Tick,
}

#[derive(Deserialize, Debug)]
pub struct SnapshotData {
    #[serde(rename = "as")]
    as_: Vec<(String, String, String)>,
    bs: Vec<(String, String, String)>,
}

#[derive(Deserialize, Debug)]
pub struct UpdateData(Vec<Vec<String>>);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum MessageData {
    Ask { a: UpdateData },
    Bid { b: UpdateData },
    Snap(SnapshotData),
}

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub bids: BTreeMap<String, f32>,
    pub asks: BTreeMap<String, f32>,
    max_depth: u16,
}

impl OrderBook {
    pub fn new(depth: u16) -> OrderBook {
        OrderBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            max_depth: depth,
        }
    }

    pub fn initalize(&mut self, snapshot: &SnapshotData) {
        for (price, vol, _) in snapshot.bs.iter() {
            self.bids.insert(price.to_owned(), vol.parse().unwrap());
        }
        for (price, vol, _) in snapshot.as_.iter() {
            self.asks.insert(price.to_owned(), vol.parse().unwrap());
        }
    }

    pub fn update_asks(&mut self, updates: &UpdateData) {
        for ask in updates.0.iter() {
            let (price, vol) = (ask[0].to_owned(), ask[1].parse().unwrap());
            if vol != 0f32 {
                self.asks.insert(price, vol);
                while self.asks.len() > self.max_depth as usize {
                    self.asks.pop_last();
                }
            } else {
                self.asks.remove(&price);
            }
        }
    }

    pub fn update_bids(&mut self, updates: &UpdateData) {
        for bid in updates.0.iter() {
            let (price, vol) = (bid[0].to_owned(), bid[1].parse().unwrap());
            if vol != 0f32 {
                self.bids.insert(price, vol);
                while self.bids.len() > self.max_depth as usize {
                    self.bids.pop_first();
                }
            } else {
                self.bids.remove(&price);
            }
        }
    }
}
