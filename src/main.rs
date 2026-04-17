// Count reachable positions in Dobutsu Shogi from the initial position.
//
// Terminal positions (no successors):
//   - forced-win: Black can capture White Lion
//   - forced-loss: White Lion at row 3, Black cannot capture
//
// Chick drops on row 0 ARE allowed (chick sits immovable until captured).

use rustc_hash::FxHashSet;
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

fn generate_successors(pos: u64, out: &mut Vec<u64>) {
    // Board moves
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
            out.push(np);
        }
    }
    // Drops (Chick drop on row 0 is allowed)
    for hi in 0..3 {
        let cnt = get_hand(pos, hi);
        if cnt == 0 { continue; }
        let drop_piece: u32 = match hi { 0 => 2, 1 => 3, 2 => 4, _ => unreachable!() };
        for sq in 0..NUM_SQ {
            if get_sq(pos, sq) != 0 { continue; }
            let mut np = pos;
            np = set_sq(np, sq, drop_piece);
            np = set_hand(np, hi, cnt - 1);
            out.push(np);
        }
    }
}

fn main() {
    let init = canonical(initial_position());
    let mut visited: FxHashSet<u64> = FxHashSet::default();
    let mut all: Vec<u64> = Vec::new();
    visited.insert(init);
    all.push(init);
    let mut succ: Vec<u64> = Vec::with_capacity(80);
    let mut i: usize = 0;
    let start = Instant::now();
    let mut next_log = 1_000_000usize;

    while i < all.len() {
        let pos = all[i];
        i += 1;
        if !is_terminal(pos) {
            succ.clear();
            generate_successors(pos, &mut succ);
            for &sp in succ.iter() {
                let canon = canonical(flip_perspective(sp));
                if visited.insert(canon) {
                    all.push(canon);
                }
            }
        }
        if i >= next_log {
            eprintln!("[{:>7.1}s] processed {:>10} / discovered {:>10}",
                start.elapsed().as_secs_f64(), i, all.len());
            next_log += 1_000_000;
        }
    }
    eprintln!("done in {:.1}s", start.elapsed().as_secs_f64());
    println!("Reachable positions: {}", all.len());
}
