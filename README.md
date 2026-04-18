# Dobutsu Shogi Solver

Counts reachable positions and solves
[Dobutsu Shogi](https://en.wikipedia.org/wiki/D%C5%8Dbutsu_sh%C5%8Dgi) (animal shogi)
by retrograde analysis. Board dimensions are configurable via `ROWS` / `COLS`
constants in `src/main.rs`.

## Results

### Standard 4×3 board

+-+-+-+
|g|l|e|
+-+-+-+
| |c| |
+-+-+-+
| |C| |
+-+-+-+
|E|L|G|
+-+-+-+

**246,803,167** reachable positions from the initial position, matching Tanaka (2009).

| Category | Positions | Share |
|---|---:|---:|
| Win (for side to move) | 196,773,087 | 79.73% |
| Lose | 47,347,380 | 19.18% |
| Draw | 2,682,700 | 1.09% |
| **Total** | **246,803,167** | |

**Initial position: LOSE** — second player (Gote) wins with optimal play.

### 5×3 variant

**3,359,910,526** reachable positions — 13.6× more than 4×3 from one extra rank.

| Category | Positions | Share |
|---|---:|---:|
| Win (for side to move) | 2,597,975,993 | 77.32% |
| Lose | 683,720,498 | 20.35% |
| Draw | 78,214,035 | 2.33% |
| **Total** | **3,359,910,526** | |

**Initial position: DRAW** — neither side can force a win under the
three-fold-repetition draw rule.

## Encoding

Each position is packed into a `u128`:

| Bits | Content |
|---|---|
| `0 .. 4·NUM_SQ` | `NUM_SQ` board squares, 4 bits each |
| `4·NUM_SQ .. 4·NUM_SQ+12` | 6 hand counts (Elephant/Giraffe/Chick × 2 players), 2 bits each |

At 5×3 the board occupies 60 bits + 12 for hands = 72 bits; at 4×3, 48 + 12 = 60 bits.
`u64` was sufficient up through 4×3; `u128` is used uniformly to support larger boards.

Side-to-move is fixed to Black via vertical flip + color swap after each move.
Left-right mirror positions are identified (canonical = min of position and its reflection).

## Rules implemented

- **Board**: `ROWS × COLS` (default 5×3 in source; 4×3 is the standard game).
- **Pieces**: Lion (king), Elephant (diagonal), Giraffe (orthogonal), Chick (forward only), Hen (promoted Chick, gold-general moves).
- **Promotion**: Chick promotes to Hen when entering the opponent's back rank (mandatory).
- **Capture**: captured pieces change owner; captured Hen reverts to Chick.
- **Drops**: captured pieces may be dropped on any empty square, including the back rank.
- **Terminal positions** (no successors generated):
  - *Forced-win*: side-to-move can capture the opponent's Lion.
  - *Forced-loss*: opponent's Lion sits on the side-to-move's back rank and cannot be captured (Try rule, one ply earlier than the official version).

## Algorithm

1. **BFS** from initial position using sort-merge dedup (no hash table)
2. **Classify** terminal positions (win/lose) and compute out-degrees
3. **Build reverse edges** (CSR graph) via batched sort-merge
4. **Retrograde BFS** propagating win/lose classifications backward from terminals

Approximate resources by board size:

| Board | Reachable | Total edges | Peak RAM | Wall time |
|---|---:|---:|---:|---:|
| 4×3 | 2.47·10⁸ | 9.39·10⁸ | ~12 GB | ~7 min |
| 5×3 | 3.36·10⁹ | ~10¹⁰+ | ~230 GB | ~hours |
| 6×3 | ~4-7·10¹⁰ (est.) | — | in-memory infeasible | — |

5×3 requires a large-memory machine (≥256 GB recommended). 6×3 only fits
with disk-based BFS + a streaming retrograde; see `src/bin/disk_bfs.rs`.

## Usage

Two binaries, sharing the core game logic in `src/lib.rs`.

**In-memory full solve** (BFS + retrograde). Suitable for boards up through ~5×3:

```
cargo run --release --bin dobutsu_count
```

**Disk-based BFS** for larger boards. Writes sorted position files to disk
and merges them via external sort-merge — keeps RAM use tiny regardless of
how big the state space is. Currently performs Phase 1 (reachable-position
count) only; retrograde-on-disk is future work.

```
cargo run --release --bin disk_bfs
```

Tune with env vars:
- `DISK_BFS_DIR` — work directory for on-disk files (default `./disk_bfs_work`)
- `DISK_BFS_RUN_POS` — positions per in-memory sort run (default `500_000_000`, ≈ 8 GiB). Larger values → fewer runs per level → faster merges.

Example for the 760 GB machine (use 10 GiB sort buffer, put files on fast NVMe):
```
DISK_BFS_DIR=/mnt/nvme/dobutsu DISK_BFS_RUN_POS=625000000 \
    cargo run --release --bin disk_bfs
```

To switch board size, edit `ROWS` and `COLS` in `src/lib.rs`. The initial
position derives from `ROWS`/`COLS` automatically.
