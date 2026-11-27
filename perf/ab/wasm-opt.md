# A/B: the engine built without wasm-opt (shipped) against with it

5 interleaved repeats, production build, 2026-11-20T06:25:22.585Z.

## Download

| | raw | gzip | brotli |
|---|---|---|---|
| no wasm-opt (shipped) | 15.55 MB | 4.59 MB | 2.98 MB |
| wasm-opt -O | 12.96 MB | 4.24 MB | 2.86 MB |

## The engine alone (Node, 1,000 and 5,000 paragraphs, 5 interleaved runs each)

| | engine memory 5,000 (MB) | typing per key, 1,000 (ms) | typing per key, 5,000 (ms) | open 5,000 (ms) |
|---|---|---|---|---|
| no wasm-opt (shipped) | 161.4 (161.4 to 161.4) | 2.46 (2.41 to 2.48) | 12.23 (12.14 to 12.49) | 55 (54.8 to 66) |
| wasm-opt -O | 161.4 (161.4 to 161.4) | 2.59 (2.54 to 2.64) | 11.85 (11.8 to 12.74) | 52.3 (52.3 to 53.2) |

## The whole app (footprint in MB, median and range)

| | start screen | blank document | peak while typing 100 keys | settled after typing | own work per key (ms) | time to next paint (ms) |
|---|---|---|---|---|---|---|
| no wasm-opt (shipped) | 157 (156 to 157) | 190 (190 to 191) | 193 (193 to 194) | 197 (197 to 197) | 2.5 (2.34 to 2.57) | 15.7 (15.6 to 15.9) |
| wasm-opt -O | 170 (170 to 172) | 212 (211 to 212) | 527 (523 to 580) | 223 (221 to 225) | 2.4 (2.35 to 2.55) | 15.7 (15.4 to 15.8) |

## A 1,000-paragraph document (typing in the middle, 3 interleaved runs)

| | footprint at rest (MB) | peak while typing (MB) | settled after (MB) | own work per key (ms) | engine per key (ms) |
|---|---|---|---|---|---|
| no wasm-opt (shipped) | 239 (239 to 239) | 255 (249 to 261) | 261 (255 to 266) | 7.79 (7.63 to 7.86) | 2.6 (2.6 to 2.6) |
| wasm-opt -O | 267 (265 to 267) | 571 (567 to 584) | 289 (284 to 311) | 7.87 (7.74 to 8) | 2.6 (2.6 to 2.6) |

Machine: load 7.22 on 10 cores (72% busy). **Busy: footprints and times here can be tens of MB / tens of percent off; read the ranges, and compare rows only within this one session.**
