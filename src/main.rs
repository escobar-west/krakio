mod lib;
use crate::lib::{Event, MessageData, OrderBook};
use futures_util::{FutureExt, SinkExt, StreamExt};
use getopts::Options;
use serde_json::{json, Value};
use std::{
    env, io,
    sync::{Arc, Mutex},
};
use termion::{event::Key, input::TermRead, raw::IntoRawMode};
use tokio::{
    sync::Notify,
    time::{sleep, Duration},
};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Spans,
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};

const _DEPTH_DEFAULT: u16 = 10;
const _PAIR_DEFAULT: &str = "XBT/USD";
const _DELAY_DEFAULT: u64 = 1;
type Buff = [(String, f32); _DEPTH_DEFAULT as usize];
type MemBuff = (Buff, Buff);

fn main() {
    let (pair, delay): (String, u64) = parse_args();

    let stdout = io::stdout().into_raw_mode().unwrap();
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let (tick_tx, event_rx) = std::sync::mpsc::channel();
    let mem_buff_rx: Arc<Mutex<MemBuff>> = Arc::new(Mutex::new(Default::default()));

    let key_tx = tick_tx.clone();
    let mem_buff_tx = Arc::clone(&mem_buff_rx);

    std::thread::spawn(move || {
        connect_to_websocket(pair, delay, tick_tx, mem_buff_tx);
    });
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for key in stdin.keys().flatten() {
            key_tx.send(Event::Input(key)).expect("Could not send key");
        }
    });
    while let Ok(event) = event_rx.recv() {
        match event {
            Event::Tick => {
                terminal
                    .draw(|f| {
                        let chunks = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints(
                                [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref(),
                            )
                            .split(f.size());

                        let (bid_items, ask_items): (Vec<ListItem>, Vec<ListItem>) = {
                            let mem_lock = mem_buff_rx.lock().unwrap();
                            (
                                mem_lock
                                    .0
                                    .iter()
                                    .rev()
                                    .map(|(k, v)| {
                                        let lines = vec![Spans::from(format!("{}: {}", k, v))];
                                        ListItem::new(lines)
                                            .style(Style::default().fg(Color::LightGreen))
                                    })
                                    .collect(),
                                mem_lock
                                    .1
                                    .iter()
                                    .map(|(k, v)| {
                                        let lines = vec![Spans::from(format!("{}: {}", k, v))];
                                        ListItem::new(lines)
                                            .style(Style::default().fg(Color::LightRed))
                                    })
                                    .collect(),
                            )
                        };
                        let (bid_items, ask_items) = (
                            List::new(bid_items).block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .style(Style::default().bg(Color::Black))
                                    .title("Bids"),
                            ),
                            List::new(ask_items).block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .style(Style::default().bg(Color::Black))
                                    .title("Asks"),
                            ),
                        );
                        f.render_widget(bid_items, chunks[0]);
                        f.render_widget(ask_items, chunks[1]);
                    })
                    .expect("Could not draw to terminal");
            }
            Event::Input(key) => {
                if let Key::Char('q') = key {
                    return;
                }
            }
        }
    }
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    println!("{}", opts.usage(&brief));
}

fn parse_args() -> (String, u64) {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("p", "pair", "trading pair", "PAIR");
    opts.optopt("m", "ms", "delay in ms", "MILLISECONDS");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            panic!("{}", f)
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        std::process::exit(0);
    }
    let pair = matches
        .opt_str("p")
        .unwrap_or_else(|| _PAIR_DEFAULT.to_string());
    let delay = matches.opt_str("m").map_or(_DELAY_DEFAULT, |x| {
        x.parse().expect("Could not parse delay as u32")
    });
    (pair, delay)
}

#[tokio::main]
async fn connect_to_websocket(
    pair: String,
    delay: u64,
    tick_tx: std::sync::mpsc::Sender<Event>,
    mem_buff_tx: Arc<Mutex<MemBuff>>,
) {
    let mut book = OrderBook::new(_DEPTH_DEFAULT);
    tick_tx.send(Event::Tick).expect("Could not send orderbook");

    let url = url::Url::parse("wss://ws.kraken.com/").unwrap();
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    let (mut write, mut read) = ws_stream.split();

    let alarm_rx = Arc::new(Notify::new());
    let alarm_tx = alarm_rx.clone();

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_millis(delay)).await;
            alarm_tx.notify_one();
        }
    });
    let send_msg = write.send(Message::text(
        json!({
            "event": "subscribe",
            "subscription": {
                "name": "book",
                "depth": _DEPTH_DEFAULT
            },
            "pair": [pair]
        })
        .to_string(),
    ));
    send_msg.await.expect("send message failed");

    loop {
        if let Some(Ok(Message::Text(s))) = read.next().await {
            let mut json_data: Value = serde_json::from_str(&s).expect("Could not parse JSON");
            if let Some(v) = json_data.as_array_mut() {
                for elem in v.drain(1..).filter(|x| x.is_object()) {
                    let data: MessageData =
                        serde_json::from_value(elem).expect("Could not parse book message");
                    match data {
                        MessageData::Ask { a: asks, .. } => {
                            book.update_asks(&asks);
                        }
                        MessageData::Bid { b: bids, .. } => {
                            book.update_bids(&bids);
                        }
                        MessageData::Snap(snapshot) => {
                            book.initalize(&snapshot);
                        }
                    }
                }
            } else if let Some(o) = json_data.as_object() {
                match o.get("errorMessage") {
                    None => {}
                    Some(m) => panic!("fatal error occured: {}", m),
                }
            }
        }
        if alarm_rx.notified().now_or_never().is_some() {
            let mut mem_lock = mem_buff_tx.lock().unwrap();
            for (i, (bid, ask)) in book.bids.iter().zip(book.asks.iter()).enumerate() {
                mem_lock.0[i] = (bid.0.to_owned(), *bid.1 as f32);
                mem_lock.1[i] = (ask.0.to_owned(), *ask.1 as f32);
            }
            tick_tx.send(Event::Tick).expect("Could not send orderbook");
        }
    }
}
