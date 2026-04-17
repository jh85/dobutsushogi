# Dobutsu Shogi Position Counter

Counts the number of positions reachable from the initial position in
[Dobutsu Shogi](https://en.wikipedia.org/wiki/D%C5%8Dbutsu_sh%C5%8Dgi) (animal shogi),
a simplified 4×3 shogi variant.

## Result

**246,803,167** reachable positions, matching the published result by Tanaka (2009).

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
- **Drops**: captured pieces may be dropped on any empty square. Chick drops on the back rank (row 0) are allowed (the chick sits immovable until captured).
- **Terminal positions** (no successors generated):
  - *Forced-win*: side-to-move can capture the opponent's Lion.
  - *Forced-loss*: opponent's Lion is on the side-to-move's back rank and cannot be captured (Try rule).

## Usage

```
cargo run --release
```

Requires ~4 GB RAM. Runs in about 75 seconds on a modern machine (sort-merge BFS, no hash table).
