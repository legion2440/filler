# Filler

A Rust player for the **01 Edu Filler** algorithmic game with strict placement validation, opponent-oriented territory control, an integrated **Live Visualizer**, and a browser-based **Replay Lab**.

The player reads the official `game_engine` protocol from `stdin` and writes exactly one `X Y\n` move to `stdout` per turn.

· [Русская версия](README_RU.md)

## 📋 TOC

- [🎮 Game in one minute](#-game-in-one-minute)
- [🚀 Quick start](#-quick-start)
- [🐳 Official game environment](#-official-game-environment)
- [🖥️ Live Visualizer](#️-live-visualizer)
- [🧪 Manual Docker run](#-manual-docker-run)
- [🧠 Player algorithm](#-player-algorithm)
- [✅ Placement rules](#-placement-rules)
- [🎯 Strategy](#-strategy)
- [🧪 Tests](#-tests)
- [🏆 Audit checklist](#-audit-checklist)
- [🎬 Replay Lab](#-replay-lab)
- [🏗️ Architecture](#️-architecture)
- [📁 Project structure](#-project-structure)
- [⚠️ Notes](#️-notes)
- [🧑‍💻 Author](#-author)

## 🎮 Game in one minute

Filler is a turn-based territory game for two bots.

- Each player starts with its own territory on the Anfield.
- `game_engine` sends a random piece every turn.
- A legal piece must overlap **exactly one** cell of your existing territory.
- It may not overlap any opponent cell.
- Every occupied piece cell must remain inside the board.
- The remaining occupied cells become new territory.
- There is no direct capture or repainting of enemy cells.
- The tactical goal is to expand, block routes, cut the opponent away from free space, and finish with more occupied cells.

Player 1 uses `@` / `a`; Player 2 uses `$` / `s`. Lowercase characters mark the most recently placed piece.

## 🚀 Quick start

### Requirements

- Rust `1.63+`
- Cargo
- Docker Desktop / Docker Engine
- a modern browser for the visualizer
- `make` is optional

Clone:

```bash
git clone https://github.com/legion2440/filler.git
cd filler
```

Run unit tests:

```bash
cargo test
```

Build the player for the current OS:

```bash
cargo build --release --bin filler
```

For normal development and audit runs, use the integrated launcher:

```bash
make visualizer
```

or:

```bash
cargo run --bin visualizer
```

It opens `http://127.0.0.1:8080`, detects the official engine bundle, prepares the Linux Rust player inside Docker, launches selected matches, streams them to the browser, and stores replay logs automatically.

## 🐳 Official game environment

The official `game_engine`, maps and robots are **not stored in this repository**. They are supplied separately by 01 Edu.

Download:

```text
https://assets.01-edu.org/filler/filler.zip
```

Unpack the archive **next to** this repository and name the directory `filler-engine`:

```text
TSchool/
├── filler/                  <- this repository
│   ├── src/
│   ├── tests/
│   ├── visualizer/
│   └── ...
│
└── filler-engine/           <- official 01 Edu bundle
    ├── Dockerfile
    ├── linux_game_engine
    ├── linux_robots/
    ├── m1_game_engine
    ├── m1_robots/
    ├── maps/
    └── solution/
```

The Visualizer looks for `../filler-engine` automatically. If the bundle is elsewhere:

```bash
cargo run --bin visualizer -- --engine-dir "D:\path\to\filler-engine"
```

or:

```bash
FILLER_ENGINE_DIR=/path/to/filler-engine make visualizer
```

Docker must be running. On the first **Start**, the Rust server automatically:

1. validates `Dockerfile`, `linux_game_engine`, `linux_robots/` and `maps/`;
2. builds the official image as `filler` if it does not exist;
3. mounts this repository into `/filler/solution`;
4. compiles the player **inside the official Rust 1.63 Linux container** using an isolated `target/docker-linux` directory;
5. launches `linux_game_engine` with the selected map, opponent and side;
6. streams engine output to the browser through SSE;
7. writes the complete raw match log to `replays/`.

This avoids Windows/Linux executable mismatches and avoids manual `GOOS`-style cross-compilation entirely.

## 🖥️ Live Visualizer

Start:

```bash
make visualizer
```

The **Run match** panel provides:

- map selection from the real `maps/` directory;
- opponent selection from `linux_robots/`;
- `P1`, `P2`, or alternating sides;
- `1–20` games per series;
- random or fixed seed;
- Live visualization on/off;
- Start / Stop;
- ready-made audit presets.

Presets:

| Preset | Map | Opponent | Games | Side |
| --- | --- | --- | ---: | --- |
| `wall_e audit` | `map00` | `wall_e` | 5 | alternate |
| `h2_d2 audit` | `map01` | `h2_d2` | 5 | alternate |
| `bender audit` | `map02` | `bender` | 5 | alternate |
| `terminator bonus` | `map01` | `terminator` | 5 | alternate |

The main board, Play/Pause controls, step buttons, timeline and playback speed are kept directly in the primary view. The layout is designed to remain usable at **100% browser zoom** on a normal desktop display; the territory graph is placed below the primary match area.

### Replay-only mode

`visualizer/index.html` can still be opened directly. In that mode file import, playback and the local replay library work, but Docker launching and live SSE are unavailable because there is no local Rust server.

## 🧪 Manual Docker run

The Visualizer is the convenient path. These commands are the explicit fallback an auditor can use.

### 1. Build the official image

From the unpacked engine directory:

**Windows Git Bash**

```bash
cd /d/TSchool/filler-engine
docker build -t filler .
```

**WSL / Linux**

```bash
cd /mnt/d/TSchool/filler-engine
docker build -t filler .
```

The Dockerfile warning about shell-form `ENTRYPOINT` belongs to the supplied 01 Edu image and does not prevent it from building.

### 2. Enter the container with the repository mounted

**Windows Git Bash**

```bash
docker run --rm -it \
  -v "D:/TSchool/filler:/filler/solution" \
  filler
```

**WSL / Linux**

```bash
docker run --rm -it \
  -v /mnt/d/TSchool/filler:/filler/solution \
  filler
```

After this command the prompt is inside the container, for example:

```text
root@...:/filler#
```

### 3. Build the Rust player inside the container

Run **inside the container**:

```bash
cd /filler/solution
CARGO_TARGET_DIR=target/docker-linux cargo build --release --bin filler
cd /filler
```

The executable is now:

```text
/filler/solution/target/docker-linux/release/filler
```

### 4. Run matches inside the container

`wall_e`:

```bash
./linux_game_engine \
  -f maps/map00 \
  -p1 /filler/solution/target/docker-linux/release/filler \
  -p2 linux_robots/wall_e
```

`h2_d2`:

```bash
./linux_game_engine \
  -f maps/map01 \
  -p1 /filler/solution/target/docker-linux/release/filler \
  -p2 linux_robots/h2_d2
```

`bender`:

```bash
./linux_game_engine \
  -f maps/map02 \
  -p1 /filler/solution/target/docker-linux/release/filler \
  -p2 linux_robots/bender
```

Bonus `terminator`:

```bash
./linux_game_engine \
  -f maps/map01 \
  -p1 /filler/solution/target/docker-linux/release/filler \
  -p2 linux_robots/terminator
```

Swap `-p1` and `-p2` to test the student player on the other side.

## 🧠 Player algorithm

The Rust implementation separates protocol parsing, legality and strategy so the critical rules remain testable in isolation.

Each turn:

1. parse the Anfield and incoming piece;
2. store occupied piece cells as coordinates;
3. generate candidate origins by aligning occupied piece cells with every own territory cell;
4. deduplicate those origins;
5. reject illegal placements;
6. build a multi-source distance field from opponent territory;
7. fast-score every legal move;
8. deep-evaluate the strongest candidates;
9. choose deterministically and write `X Y\n`;
10. flush `stdout` immediately for the interactive engine protocol.

Candidate generation does not blindly scan every possible top-left origin on the map.

## ✅ Placement rules

`validate_placement` is strategy-independent:

- `0` own overlaps → invalid;
- exactly `1` own overlap → valid if all other checks pass;
- `2+` own overlaps → invalid;
- any opponent overlap → invalid;
- any occupied piece cell outside the board → invalid.

Only occupied cells matter. Empty `.` padding may extend beyond an edge if every occupied piece cell is still inside the Anfield. The parser accepts both `O` and `#` style piece markers because every non-`.` piece character is treated as occupied.

## 🎯 Strategy

The strategy is a deterministic two-stage heuristic port of the previously tuned algorithm.

### Fast score

Each legal move considers:

- minimum distance from newly occupied cells to the opponent;
- aggregate opponent distance;
- pressure near enemy territory;
- future frontier cells;
- newly gained cells;
- light edge and center adjustments.

When territories are far apart, approach pressure is weighted more strongly. Once contact is close, territory/frontier value receives more weight.

### Deep score

The strongest fast-scored candidates are evaluated again using:

- projected distance-based territory control;
- projected own frontier;
- projected opponent frontier.

Only the top `72` candidates enter the deep stage to keep per-turn work bounded. Ties use a deterministic coordinate order so seeded runs remain comparable.

## 🧪 Tests

Run:

```bash
cargo test
```

`tests/core.rs` covers the assignment/audit requirements:

- `p1` and `p2` parsing;
- Anfield dimensions and rows;
- piece dimensions and occupied cells;
- exactly-one-overlap validation;
- zero-overlap rejection;
- two-own-cell rejection;
- opponent overlap rejection;
- occupied-cell boundary rejection;
- empty padding outside a boundary;
- generated placements being legal;
- exact `X Y\n` output;
- strategy producing a legal move;
- no-move behavior;
- Player 2 symbols and placements.

Useful local checks before submission:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## 🏆 Audit checklist

The official audit requires:

| Map | Opponent | Required result |
| --- | --- | --- |
| `map00` | `wall_e` | at least 4 wins out of 5 |
| `map01` | `h2_d2` | at least 4 wins out of 5 |
| `map02` | `bender` | at least 4 wins out of 5 |
| suitable map | `terminator` | bonus: at least 4 wins out of 5 |

The student player must be exercised as both `p1` and `p2`.

The Rust implementation intentionally does **not** hard-code unverified win-rate claims in this README. The strategy was ported from the tuned implementation, but the Rust binary should be re-run through the official Docker matrix before submission and the measured results recorded afterward.

## 🎬 Replay Lab

Every match launched from the UI writes a raw `.log` under `replays/`. When a match ends, the browser loads the same file as a replay.

Features:

- live Canvas rendering;
- drag & drop raw `game_engine` logs;
- normalized Replay JSON import/export;
- built-in demo;
- Play / Pause;
- first / previous / next / last turn;
- timeline scrubbing;
- `0.5×`, `1×`, `2×`, `4×`, `8×` playback;
- current piece preview;
- territory scores and growth graph;
- browser-local replay library;
- final-board previews;
- raw logs retained on disk even if browser storage is full.

Manual replay capture is also available from inside `/filler`:

```bash
sh /filler/solution/scripts/capture-replay.sh \
  -f maps/map00 \
  -p1 /filler/solution/target/docker-linux/release/filler \
  -p2 linux_robots/wall_e
```

## 🏗️ Architecture

```text
                         official game_engine
                                 |
                         stdin / stdout
                                 |
                                 v
                    +-------------------------+
                    |       Rust player       |
                    | parser -> validator     |
                    | -> strategy -> X Y\n    |
                    +-------------------------+

 browser
    |
    | HTTP / SSE
    v
 +---------------------------+
 | Rust visualizer server    |
 | match presets / launcher  |
 +-------------+-------------+
               |
               | docker CLI
               v
 +---------------------------+
 | official filler image     |
 | linux_game_engine         |
 | Rust player + robot       |
 +-------------+-------------+
               |
        raw engine output
          /           \
         v             v
     SSE live UI    replays/*.log
         |             |
         +-------> Replay parser
                       |
                       v
                  Canvas playback
```

The player never writes debug or visualizer data to `stdout`; only the move protocol goes there. Diagnostics use `stderr`.

## 📁 Project structure

```text
filler/
├── scripts/
│   └── capture-replay.sh
├── src/
│   ├── bin/
│   │   └── visualizer.rs
│   ├── lib.rs
│   ├── main.rs
│   ├── model.rs
│   ├── output.rs
│   ├── parser.rs
│   ├── placement.rs
│   └── strategy.rs
├── tests/
│   └── core.rs
├── visualizer/
│   ├── app.js
│   ├── index.html
│   ├── renderer.js
│   ├── replay.js
│   └── styles.css
├── .gitignore
├── Cargo.toml
├── Makefile
├── README.md
└── README_RU.md
```

`target/` and generated `replays/` are ignored by Git.

## ⚠️ Notes

- The official bundle currently uses `rust:1.63-buster`; the player is therefore kept compatible with Rust `1.63` and has no third-party Rust dependencies.
- The integrated launcher compiles the Linux player inside the official image rather than attempting Windows-to-Linux linking on the host.
- Future pieces are random, so the strategy does not perform deterministic minimax over unknown future pieces.
- `0 0\n` is returned when no legal placement exists, matching the assignment protocol expectation that the bot must still answer.
- The visualizer server binds to `127.0.0.1` by default and is intended for local development/audit use.

## 🧑‍💻 Author

- Nazar Yestayev (@nyestaye)
