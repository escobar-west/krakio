# krakio
(Kind of) async orderbook pulled and managed from Kraken public websocket API.

```cargo run --release [-p PAIR][-m TICK_RATE]```

pair: 'BTC/USD' or 'ETH/USD' or 'BTC/ETH' for example. Defaults to 'BTC/USD'. Many trading pairs are available, just make sure to use upper case (TODO: parse lowercase args).

tick_rate: Number of milliseconds before terminal refreshes orderbook display. Default is 1ms.
