// Dobutsu Shogi solver: in-memory full retrograde pipeline.
// Suitable for boards up to ~5x3. For larger boards, use the disk_bfs binary.

use dobutsu_count::*;
use std::time::Instant;

fn main() {
    let t0 = Instant::now();
    eprintln!("Board: {}x{} ({} squares)", ROWS, COLS, NUM_SQ);

    eprintln!("Phase 1: BFS...");
    let known = bfs();
    let n = known.len();
    eprintln!("  {} reachable positions in {:.1}s", n, t0.elapsed().as_secs_f64());

    eprintln!("Phase 2: Classify terminals...");
    let mut status: Vec<u8> = vec![STATUS_UNKNOWN; n];
    let mut remaining: Vec<u8> = vec![0; n];
    let mut succ_buf: Vec<Pos> = Vec::new();
    let (mut win_t, mut lose_t) = (0usize, 0usize);
    for i in 0..n {
        let pos = known[i];
        if is_terminal(pos) {
            let wl = find_white_lion(pos);
            if black_can_reach(pos, wl) {
                status[i] = STATUS_WIN; win_t += 1;
            } else {
                status[i] = STATUS_LOSE; lose_t += 1;
            }
        } else {
            succ_buf.clear();
            generate_successors_canonical(pos, &mut succ_buf);
            remaining[i] = succ_buf.len() as u8;
        }
    }
    eprintln!("  terminals: {} win + {} lose = {} in {:.1}s",
        win_t, lose_t, win_t + lose_t, t0.elapsed().as_secs_f64());

    eprintln!("Phase 3: Build reverse edges...");
    let (rev_offset, rev_edges) = build_reverse_csr(&known, &status);
    eprintln!("  {} reverse edges in {:.1}s", rev_edges.len(), t0.elapsed().as_secs_f64());

    eprintln!("Phase 4: Retrograde analysis...");
    retrograde(&mut status, &mut remaining, &rev_offset, &rev_edges);
    let total_win = status.iter().filter(|&&s| s == STATUS_WIN).count();
    let total_lose = status.iter().filter(|&&s| s == STATUS_LOSE).count();
    let total_draw = n - total_win - total_lose;
    eprintln!("  {} win, {} lose, {} draw in {:.1}s",
        total_win, total_lose, total_draw, t0.elapsed().as_secs_f64());

    let init = canonical(initial_position());
    let init_idx = known.binary_search(&init).unwrap();
    let result = match status[init_idx] {
        STATUS_WIN => "WIN (first player wins)",
        STATUS_LOSE => "LOSE (second player wins)",
        _ => "DRAW",
    };

    println!("Board: {}x{}", ROWS, COLS);
    println!("Reachable positions: {}", n);
    println!("  Win:  {}", total_win);
    println!("  Lose: {}", total_lose);
    println!("  Draw: {}", total_draw);
    println!("Initial position: {}", result);
}
