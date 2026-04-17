# Dobutsu Shogi Solver

Counts reachable positions and solves
[Dobutsu Shogi](https://en.wikipedia.org/wiki/D%C5%8Dbutsu_sh%C5%8Dgi) (animal shogi)
by retrograde analysis.

## Results

**246,803,167** reachable positions from the initial position, matching Tanaka (2009).

| Category | Positions |
|---|---|
| Win (for side to move) | 196,773,087 |
| Lose | 47,347,380 |
| Draw | 2,682,700 |
| **Total** | **246,803,167** |

**Initial position: LOSE** — second player wins with optimal play (up to 78 plies).

## Encoding

Each position is packed into 60 bits of a `u64`:

| Bits   | Content                                         |
|--------|-------------------------------------------------|
| 0–47   | 12 board squares, 4 bits each (row×3 + col)     |
| 48–59  | 6 hand counts, 2 bits each (Elephant/Giraffe/Chick × 2 players) |

Side-to-move is fixed to Black via vertical flip + color swap after each move.
Left-right mirror positions are identified (canonical = min of position and its reflection).

## Rules implemented

- **Board**: 4 rows × 3 columns.
- **Pieces**: Lion (king), Elephant (diagonal), Giraffe (orthogonal), Chick (forward only), Hen (promoted Chick, gold-general moves).
- **Promotion**: Chick promotes to Hen when entering row 0 (mandatory).
- **Capture**: captured pieces change owner; captured Hen reverts to Chick.
- **Drops**: captured pieces may be dropped on any empty square, including the back rank.
- **Terminal positions** (no successors generated):
  - *Forced-win*: side-to-move can capture the opponent's Lion.
  - *Forced-loss*: opponent's Lion is on the side-to-move's back rank and cannot be captured (Try rule).

## Algorithm

1. **BFS** from initial position using sort-merge dedup (no hash table) — 74s
2. **Classify** terminal positions (win/lose) and compute out-degrees — 35s
3. **Build reverse edges** (CSR graph) via batched sort-merge — 260s
4. **Retrograde BFS** to propagate win/lose classifications — 18s

Total: ~6.5 minutes on a modern machine. Requires ~12 GB RAM.

## Usage

```
cargo run --release
```
