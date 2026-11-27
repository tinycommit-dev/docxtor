# A/B: the menu bar and toolbar drawing once after start (commit a1036a4)

7 interleaved repeats, production build, 2026-11-20T06:34:21.393Z. The two builds are the same source but for that commit's change to packages/editor/src (the variant has its own copy of the packages).

| | DOM nodes added during startup (proof the change is in) | start screen footprint (MB) | blank document footprint (MB) | page load to start screen (ms) | navigation until a blank document takes a key (ms) |
|---|---|---|---|---|---|
| draws once (shipped, a1036a4) | 6076 (6076 to 6076) | 157 (157 to 157) | 191 (190 to 191) | 516 (503 to 533) | 665 (663 to 670) |
| draws on every change (a1036a4 undone) | 16707 (16707 to 16707) | 216 (215 to 216) | 253 (253 to 254) | 2946 (2871 to 3149) | 3085 (3032 to 3189) |

Machine: load 8.73 on 10 cores (87% busy). **Busy: footprints and times here can be tens of MB / tens of percent off; read the ranges, and compare rows only within this one session.**
