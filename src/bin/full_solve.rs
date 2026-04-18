// Full solve: compute WDL for every canonical initial position.
//
// Pipeline:
//   1. Enumerate all initial positions (8 distinct pieces on NUM_SQ squares,
//      hand counts zero). Canonicalize (left-right reflection) and dedup.
//   2. Seed BFS from the dedup'd seeds; close the state space under moves.
//   3. Classify terminals; build reverse CSR; retrograde.
//   4. Binary-search each seed in the solved table and emit CSV.
//
// Output CSV columns: position,wdl
//   position: decimal encoding of the canonical Pos (fits in u64 for 4x3)
//   wdl:      single char, W = Black wins, D = draw, L = Black loses
//
// Env:
//   FULL_SOLVE_OUT   output file path (default ./initials_{ROWS}x{COLS}.csv)

use dobutsu_count::*;
use rayon::prelude::*;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

fn enumerate_initial_positions() -> Vec<Pos> {
    // 8 pieces: BLion=1, BElephant=2, BGiraffe=3, BChick=4,
    //           WLion=6, WElephant=7, WGiraffe=8, WChick=9.
    // Pieces are distinguishable (one of each type per player).
    const PIECES: [u32; 8] = [1, 2, 3, 4, 6, 7, 8, 9];
    let n_sq = NUM_SQ;

    fn recurse(
        pos: Pos,
        used: u64,
        pi: usize,
        n_sq: usize,
        out: &mut Vec<Pos>,
    ) {
        if pi == PIECES.len() {
            out.push(canonical(pos));
            return;
        }
        for sq in 0..n_sq {
            if (used >> sq) & 1 != 0 { continue; }
            let np = set_sq(pos, sq, PIECES[pi]);
            recurse(np, used | (1u64 << sq), pi + 1, n_sq, out);
        }
    }

    // Parallelize on outermost piece (black lion) to get multi-core enumeration.
    let mut out: Vec<Pos> = (0..n_sq)
        .into_par_iter()
        .flat_map_iter(|lion_sq| {
            let mut local = Vec::new();
            let pos = set_sq(0, lion_sq, PIECES[0]);
            recurse(pos, 1u64 << lion_sq, 1, n_sq, &mut local);
            local.into_iter()
        })
        .collect();

    out.par_sort_unstable();
    out.dedup();
    out
}

fn diff_sorted_par(known: &[Pos], candidates: &[Pos]) -> Vec<Pos> {
    candidates
        .par_chunks(65_536)
        .flat_map_iter(|chunk| {
            let mut local = Vec::with_capacity(chunk.len());
            for &c in chunk {
                if known.binary_search(&c).is_err() {
                    local.push(c);
                }
            }
            local.into_iter()
        })
        .collect()
}

fn bfs_from_seeds(mut known: Vec<Pos>) -> Vec<Pos> {
    let t0 = Instant::now();
    known.par_sort_unstable();
    known.dedup();
    let mut frontier = known.clone();
    eprintln!(
        "[{:>7.1}s]   seed frontier: {} positions",
        t0.elapsed().as_secs_f64(), frontier.len()
    );

    for level in 1.. {
        let mut succs: Vec<Pos> = frontier
            .par_chunks(8_192)
            .flat_map_iter(|chunk| {
                let mut local = Vec::new();
                let mut buf = Vec::new();
                for &pos in chunk {
                    if is_terminal(pos) { continue; }
                    buf.clear();
                    generate_successors_canonical(pos, &mut buf);
                    local.extend_from_slice(&buf);
                }
                local.into_iter()
            })
            .collect();
        if succs.is_empty() { break; }

        succs.par_sort_unstable();
        succs.dedup();

        let new_positions = diff_sorted_par(&known, &succs);
        drop(succs);
        if new_positions.is_empty() { break; }

        merge_into(&mut known, &new_positions);
        eprintln!(
            "[{:>7.1}s]   level {:>3}: +{:>12}  total={:>12}",
            t0.elapsed().as_secs_f64(), level,
            new_positions.len(), known.len()
        );
        frontier = new_positions;
    }
    known
}

fn classify_terminals_par(known: &[Pos]) -> (Vec<u8>, Vec<u8>) {
    let n = known.len();
    let mut status: Vec<u8> = vec![STATUS_UNKNOWN; n];
    let mut remaining: Vec<u8> = vec![0u8; n];

    status
        .par_iter_mut()
        .zip(remaining.par_iter_mut())
        .zip(known.par_iter())
        .for_each(|((st, rem), &pos)| {
            if is_terminal(pos) {
                let wl = find_white_lion(pos);
                *st = if black_can_reach(pos, wl) { STATUS_WIN } else { STATUS_LOSE };
            } else {
                let mut buf = Vec::new();
                generate_successors_canonical(pos, &mut buf);
                *rem = buf.len() as u8;
            }
        });

    (status, remaining)
}

fn main() {
    let t0 = Instant::now();
    eprintln!("Full-solve: board {}x{} ({} squares)", ROWS, COLS, NUM_SQ);
    eprintln!("Threads: {}", rayon::current_num_threads());

    eprintln!("[{:>7.1}s] Enumerating initial positions...", t0.elapsed().as_secs_f64());
    let seeds = enumerate_initial_positions();
    eprintln!(
        "[{:>7.1}s]   {} canonical initials",
        t0.elapsed().as_secs_f64(), seeds.len()
    );

    eprintln!("[{:>7.1}s] Phase 1: BFS from seed set (parallel)...", t0.elapsed().as_secs_f64());
    let known = bfs_from_seeds(seeds.clone());
    eprintln!(
        "[{:>7.1}s]   {} positions in closed state space",
        t0.elapsed().as_secs_f64(), known.len()
    );

    eprintln!("[{:>7.1}s] Phase 2: Classify terminals (parallel)...", t0.elapsed().as_secs_f64());
    let (mut status, mut remaining) = classify_terminals_par(&known);

    eprintln!("[{:>7.1}s] Phase 3: Build reverse CSR...", t0.elapsed().as_secs_f64());
    let (rev_offset, rev_edges) = build_reverse_csr(&known, &status);
    eprintln!(
        "[{:>7.1}s]   {} reverse edges",
        t0.elapsed().as_secs_f64(), rev_edges.len()
    );

    eprintln!("[{:>7.1}s] Phase 4: Retrograde...", t0.elapsed().as_secs_f64());
    retrograde(&mut status, &mut remaining, &rev_offset, &rev_edges);

    let out_path = std::env::var("FULL_SOLVE_OUT")
        .unwrap_or_else(|_| format!("initials_{}x{}.csv", ROWS, COLS));
    eprintln!("[{:>7.1}s] Writing CSV to {}...", t0.elapsed().as_secs_f64(), out_path);

    let mut w = BufWriter::with_capacity(1 << 20, File::create(&out_path).expect("create CSV"));
    writeln!(w, "position,wdl").unwrap();
    let (mut win_ct, mut lose_ct, mut draw_ct) = (0usize, 0usize, 0usize);
    for &seed in &seeds {
        let idx = known.binary_search(&seed).expect("seed must be in known after BFS");
        let wdl = match status[idx] {
            STATUS_WIN => { win_ct += 1; 'W' }
            STATUS_LOSE => { lose_ct += 1; 'L' }
            _ => { draw_ct += 1; 'D' }
        };
        writeln!(w, "{},{}", seed, wdl).unwrap();
    }
    w.flush().unwrap();

    eprintln!("[{:>7.1}s] Done.", t0.elapsed().as_secs_f64());
    println!("Board: {}x{}", ROWS, COLS);
    println!("Canonical initial positions: {}", seeds.len());
    println!("  Win (Black): {}", win_ct);
    println!("  Draw:        {}", draw_ct);
    println!("  Lose (Black):{}", lose_ct);
    println!("State space solved: {}", known.len());
    println!("CSV: {}", out_path);
    println!("Total wall time: {:.1}s", t0.elapsed().as_secs_f64());
}
