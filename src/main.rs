// Count reachable positions in Dobutsu Shogi from the initial position.
//
// BFS level by level with sort-merge dedup (no hash table).
//
// Terminal positions (no successors):
//   - forced-win: Black can capture White Lion
//   - forced-loss: White Lion at row 3, Black cannot capture
//
// Chick drops on row 0 ARE allowed.

use std::time::Instant;

const ROWS: i32 = 4;
const COLS: i32 = 3;
const NUM_SQ: usize = (ROWS * COLS) as usize;
const HAND_SHIFT: usize = 48;

#[inline]
fn sq_idx(r: i32, c: i32) -> usize { (r * COLS + c) as usize }
#[inline]
fn get_sq(pos: u64, sq: usize) -> u32 { ((pos >> (sq * 4)) & 0xF) as u32 }
#[inline]
fn set_sq(pos: u64, sq: usize, val: u32) -> u64 {
    (pos & !(0xFu64 << (sq * 4))) | ((val as u64) << (sq * 4))
}
#[inline]
fn get_hand(pos: u64, idx: usize) -> u32 { ((pos >> (HAND_SHIFT + idx * 2)) & 0x3) as u32 }
#[inline]
fn set_hand(pos: u64, idx: usize, val: u32) -> u64 {
    (pos & !(0x3u64 << (HAND_SHIFT + idx * 2))) | ((val as u64) << (HAND_SHIFT + idx * 2))
}

fn reflect_lr(pos: u64) -> u64 {
    let board_mask: u64 = (1u64 << HAND_SHIFT) - 1;
    let mut new_pos = pos & !board_mask;
    for r in 0..ROWS {
        for c in 0..COLS {
            new_pos = set_sq(new_pos, sq_idx(r, COLS - 1 - c), get_sq(pos, sq_idx(r, c)));
        }
    }
    new_pos
}

#[inline]
fn canonical(pos: u64) -> u64 {
    let r = reflect_lr(pos);
    if pos < r { pos } else { r }
}

fn flip_perspective(pos: u64) -> u64 {
    let mut new_pos = 0u64;
    for r in 0..ROWS {
        for c in 0..COLS {
            let p = get_sq(pos, sq_idx(r, c));
            let np = if p == 0 { 0 } else if p <= 5 { p + 5 } else { p - 5 };
            new_pos = set_sq(new_pos, sq_idx(ROWS - 1 - r, c), np);
        }
    }
    for i in 0..3 {
        let b = get_hand(pos, i);
        let w = get_hand(pos, 3 + i);
        new_pos = set_hand(new_pos, i, w);
        new_pos = set_hand(new_pos, 3 + i, b);
    }
    new_pos
}

fn initial_position() -> u64 {
    let mut p = 0u64;
    p = set_sq(p, sq_idx(3, 1), 1);
    p = set_sq(p, sq_idx(3, 2), 2);
    p = set_sq(p, sq_idx(3, 0), 3);
    p = set_sq(p, sq_idx(2, 1), 4);
    p = set_sq(p, sq_idx(0, 1), 6);
    p = set_sq(p, sq_idx(0, 0), 7);
    p = set_sq(p, sq_idx(0, 2), 8);
    p = set_sq(p, sq_idx(1, 1), 9);
    p
}

fn black_offsets(pt: u32) -> &'static [(i32, i32)] {
    match pt {
        1 => &[(-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,-1),(1,0),(1,1)],
        2 => &[(-1,-1),(-1,1),(1,-1),(1,1)],
        3 => &[(-1,0),(1,0),(0,-1),(0,1)],
        4 => &[(-1,0)],
        5 => &[(-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,0)],
        _ => &[],
    }
}

fn find_white_lion(pos: u64) -> usize {
    for sq in 0..NUM_SQ {
        if get_sq(pos, sq) == 6 { return sq; }
    }
    panic!("no white lion");
}

fn black_can_reach(pos: u64, target_sq: usize) -> bool {
    let tr = (target_sq / 3) as i32;
    let tc = (target_sq % 3) as i32;
    for sq in 0..NUM_SQ {
        let p = get_sq(pos, sq);
        if (1..=5).contains(&p) {
            let r = (sq / 3) as i32;
            let c = (sq % 3) as i32;
            for &(dr, dc) in black_offsets(p) {
                if r + dr == tr && c + dc == tc { return true; }
            }
        }
    }
    false
}

#[inline]
fn is_terminal(pos: u64) -> bool {
    let wl = find_white_lion(pos);
    if black_can_reach(pos, wl) { return true; }
    wl / 3 == 3
}

fn generate_successors_canonical(pos: u64, out: &mut Vec<u64>) {
    // Board moves — each successor is immediately flip+canonical'd
    for sq in 0..NUM_SQ {
        let p = get_sq(pos, sq);
        if !(1..=5).contains(&p) { continue; }
        let r = (sq / 3) as i32;
        let c = (sq % 3) as i32;
        for &(dr, dc) in black_offsets(p) {
            let nr = r + dr;
            let nc = c + dc;
            if nr < 0 || nr >= ROWS || nc < 0 || nc >= COLS { continue; }
            let nsq = (nr * COLS + nc) as usize;
            let target = get_sq(pos, nsq);
            if (1..=5).contains(&target) { continue; }
            if target == 6 { continue; }

            let mut np = pos;
            np = set_sq(np, sq, 0);
            let new_piece = if p == 4 && nr == 0 { 5 } else { p };
            np = set_sq(np, nsq, new_piece);

            if (7..=10).contains(&target) {
                let captured_type = target - 5;
                let hi = match captured_type {
                    2 => 0, 3 => 1, 4 | 5 => 2, _ => unreachable!(),
                };
                np = set_hand(np, hi, get_hand(np, hi) + 1);
            }
            out.push(canonical(flip_perspective(np)));
        }
    }
    // Drops
    for hi in 0..3 {
        let cnt = get_hand(pos, hi);
        if cnt == 0 { continue; }
        let drop_piece: u32 = match hi { 0 => 2, 1 => 3, 2 => 4, _ => unreachable!() };
        for sq in 0..NUM_SQ {
            if get_sq(pos, sq) != 0 { continue; }
            let mut np = pos;
            np = set_sq(np, sq, drop_piece);
            np = set_hand(np, hi, cnt - 1);
            out.push(canonical(flip_perspective(np)));
        }
    }
}

// Merge `new_sorted` (sorted, non-empty) into `known` (sorted) in-place.
fn merge_into(known: &mut Vec<u64>, new_sorted: &[u64]) {
    let old_len = known.len();
    known.resize(old_len + new_sorted.len(), 0);
    let mut i = old_len;
    let mut j = new_sorted.len();
    let mut k = known.len();
    while i > 0 && j > 0 {
        k -= 1;
        if known[i - 1] >= new_sorted[j - 1] {
            known[k] = known[i - 1];
            i -= 1;
        } else {
            known[k] = new_sorted[j - 1];
            j -= 1;
        }
    }
    while j > 0 {
        k -= 1;
        known[k] = new_sorted[j - 1];
        j -= 1;
    }
}

// Find elements in `candidates` (sorted, deduped) that are NOT in `known` (sorted).
fn diff_sorted(known: &[u64], candidates: &[u64], out: &mut Vec<u64>) {
    out.clear();
    let mut ki = 0;
    for &c in candidates {
        while ki < known.len() && known[ki] < c {
            ki += 1;
        }
        if ki >= known.len() || known[ki] != c {
            out.push(c);
        }
    }
}

fn main() {
    let init = canonical(initial_position());
    let mut known: Vec<u64> = vec![init];
    let mut frontier: Vec<u64> = vec![init];
    let mut raw_succs: Vec<u64> = Vec::new();
    let mut new_positions: Vec<u64> = Vec::new();

    let start = Instant::now();

    for level in 1.. {
        raw_succs.clear();
        for &pos in &frontier {
            if !is_terminal(pos) {
                generate_successors_canonical(pos, &mut raw_succs);
            }
        }

        if raw_succs.is_empty() {
            break;
        }

        raw_succs.sort_unstable();
        raw_succs.dedup();

        diff_sorted(&known, &raw_succs, &mut new_positions);

        if new_positions.is_empty() {
            break;
        }

        merge_into(&mut known, &new_positions);

        eprintln!(
            "[{:>7.1}s] level {:>3}: +{:>10}  = {:>10} total  (raw succs: {:>10})",
            start.elapsed().as_secs_f64(),
            level,
            new_positions.len(),
            known.len(),
            raw_succs.len()
        );

        std::mem::swap(&mut frontier, &mut new_positions);
    }

    eprintln!("done in {:.1}s", start.elapsed().as_secs_f64());
    println!("Reachable positions: {}", known.len());
}
