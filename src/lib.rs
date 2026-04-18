// Dobutsu Shogi core: position encoding, move generation, in-memory BFS, and retrograde.
//
// Board dimensions and initial position are configurable via the constants below.
// Encoding: 4 bits per square + 2 bits per hand count, packed into u128.

pub const ROWS: i32 = 5;
pub const COLS: i32 = 3;
pub const NUM_SQ: usize = (ROWS * COLS) as usize;
pub const HAND_SHIFT: usize = NUM_SQ * 4;
pub const HOME_ROW: usize = (ROWS - 1) as usize;

pub type Pos = u128;
pub type EdgeIdx = u64;

#[inline]
pub fn sq_idx(r: i32, c: i32) -> usize { (r * COLS + c) as usize }
#[inline]
pub fn get_sq(pos: Pos, sq: usize) -> u32 { ((pos >> (sq * 4)) & 0xF) as u32 }
#[inline]
pub fn set_sq(pos: Pos, sq: usize, val: u32) -> Pos {
    (pos & !(0xFu128 << (sq * 4))) | ((val as Pos) << (sq * 4))
}
#[inline]
pub fn get_hand(pos: Pos, idx: usize) -> u32 { ((pos >> (HAND_SHIFT + idx * 2)) & 0x3) as u32 }
#[inline]
pub fn set_hand(pos: Pos, idx: usize, val: u32) -> Pos {
    (pos & !(0x3u128 << (HAND_SHIFT + idx * 2))) | ((val as Pos) << (HAND_SHIFT + idx * 2))
}

pub fn reflect_lr(pos: Pos) -> Pos {
    let board_mask: Pos = (1u128 << HAND_SHIFT) - 1;
    let mut new_pos = pos & !board_mask;
    for r in 0..ROWS {
        for c in 0..COLS {
            new_pos = set_sq(new_pos, sq_idx(r, COLS - 1 - c), get_sq(pos, sq_idx(r, c)));
        }
    }
    new_pos
}

#[inline]
pub fn canonical(pos: Pos) -> Pos {
    let r = reflect_lr(pos);
    if pos < r { pos } else { r }
}

pub fn flip_perspective(pos: Pos) -> Pos {
    let mut new_pos: Pos = 0;
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

pub fn initial_position() -> Pos {
    let mut p: Pos = 0;
    // Black side (bottom)
    p = set_sq(p, sq_idx(ROWS - 1, 1), 1); // Lion
    p = set_sq(p, sq_idx(ROWS - 1, 2), 2); // Elephant
    p = set_sq(p, sq_idx(ROWS - 1, 0), 3); // Giraffe
    p = set_sq(p, sq_idx(ROWS - 2, 1), 4); // Chick
    // White side (top)
    p = set_sq(p, sq_idx(0, 1), 6); // Lion
    p = set_sq(p, sq_idx(0, 0), 7); // Elephant
    p = set_sq(p, sq_idx(0, 2), 8); // Giraffe
    p = set_sq(p, sq_idx(1, 1), 9); // Chick
    p
}

pub fn black_offsets(pt: u32) -> &'static [(i32, i32)] {
    match pt {
        1 => &[(-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,-1),(1,0),(1,1)],
        2 => &[(-1,-1),(-1,1),(1,-1),(1,1)],
        3 => &[(-1,0),(1,0),(0,-1),(0,1)],
        4 => &[(-1,0)],
        5 => &[(-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,0)],
        _ => &[],
    }
}

pub fn find_white_lion(pos: Pos) -> usize {
    for sq in 0..NUM_SQ {
        if get_sq(pos, sq) == 6 { return sq; }
    }
    panic!("no white lion");
}

pub fn black_can_reach(pos: Pos, target_sq: usize) -> bool {
    let tr = (target_sq / COLS as usize) as i32;
    let tc = (target_sq % COLS as usize) as i32;
    for sq in 0..NUM_SQ {
        let p = get_sq(pos, sq);
        if (1..=5).contains(&p) {
            let r = (sq / COLS as usize) as i32;
            let c = (sq % COLS as usize) as i32;
            for &(dr, dc) in black_offsets(p) {
                if r + dr == tr && c + dc == tc { return true; }
            }
        }
    }
    false
}

#[inline]
pub fn is_terminal(pos: Pos) -> bool {
    let wl = find_white_lion(pos);
    if black_can_reach(pos, wl) { return true; }
    wl / COLS as usize == HOME_ROW
}

pub fn generate_successors_canonical(pos: Pos, out: &mut Vec<Pos>) {
    for sq in 0..NUM_SQ {
        let p = get_sq(pos, sq);
        if !(1..=5).contains(&p) { continue; }
        let r = (sq / COLS as usize) as i32;
        let c = (sq % COLS as usize) as i32;
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

// ─── In-memory BFS (for small boards) ──────────────────────────────────────────

pub fn merge_into(known: &mut Vec<Pos>, new_sorted: &[Pos]) {
    let old_len = known.len();
    known.resize(old_len + new_sorted.len(), 0);
    let mut i = old_len;
    let mut j = new_sorted.len();
    let mut k = known.len();
    while i > 0 && j > 0 {
        k -= 1;
        if known[i - 1] >= new_sorted[j - 1] {
            known[k] = known[i - 1]; i -= 1;
        } else {
            known[k] = new_sorted[j - 1]; j -= 1;
        }
    }
    while j > 0 { k -= 1; known[k] = new_sorted[j - 1]; j -= 1; }
}

pub fn diff_sorted(known: &[Pos], candidates: &[Pos], out: &mut Vec<Pos>) {
    out.clear();
    let mut ki = 0;
    for &c in candidates {
        while ki < known.len() && known[ki] < c { ki += 1; }
        if ki >= known.len() || known[ki] != c { out.push(c); }
    }
}

pub fn bfs() -> Vec<Pos> {
    let init = canonical(initial_position());
    let mut known: Vec<Pos> = vec![init];
    let mut frontier: Vec<Pos> = vec![init];
    let mut raw_succs: Vec<Pos> = Vec::new();
    let mut new_positions: Vec<Pos> = Vec::new();

    for level in 1.. {
        raw_succs.clear();
        for &pos in &frontier {
            if !is_terminal(pos) {
                generate_successors_canonical(pos, &mut raw_succs);
            }
        }
        if raw_succs.is_empty() { break; }
        raw_succs.sort_unstable();
        raw_succs.dedup();
        diff_sorted(&known, &raw_succs, &mut new_positions);
        if new_positions.is_empty() { break; }
        merge_into(&mut known, &new_positions);
        eprintln!(
            "  level {:>3}: +{:>12}  = {:>12} total",
            level, new_positions.len(), known.len()
        );
        std::mem::swap(&mut frontier, &mut new_positions);
    }
    known
}

// ─── Reverse CSR ───────────────────────────────────────────────────────────────

pub const BATCH_CAP: usize = 80_000_000;
pub const STATUS_UNKNOWN: u8 = 0;
pub const STATUS_WIN: u8 = 1;
pub const STATUS_LOSE: u8 = 2;

fn flush_indeg_batch(known: &[Pos], in_degree: &mut [u32], batch: &mut Vec<Pos>) {
    batch.sort_unstable();
    let mut ki = 0usize;
    for &s in batch.iter() {
        while known[ki] < s { ki += 1; }
        in_degree[ki] += 1;
    }
    batch.clear();
}

struct EdgeBatch {
    succs: Vec<Pos>,
    parents: Vec<u32>,
}
impl EdgeBatch {
    fn new() -> Self { Self { succs: Vec::new(), parents: Vec::new() } }
    fn len(&self) -> usize { self.succs.len() }
    fn clear(&mut self) { self.succs.clear(); self.parents.clear(); }
    fn push(&mut self, succ: Pos, parent: u32) {
        self.succs.push(succ);
        self.parents.push(parent);
    }
}

fn flush_edge_batch(
    known: &[Pos],
    rev_edges: &mut [u32],
    write_pos: &mut [EdgeIdx],
    batch: &mut EdgeBatch,
) {
    let mut indices: Vec<u32> = (0..batch.len() as u32).collect();
    indices.sort_unstable_by_key(|&i| batch.succs[i as usize]);
    let mut ki = 0usize;
    for &idx in &indices {
        let s = batch.succs[idx as usize];
        let parent = batch.parents[idx as usize];
        while known[ki] < s { ki += 1; }
        let wp = write_pos[ki] as usize;
        rev_edges[wp] = parent;
        write_pos[ki] += 1;
    }
    batch.clear();
}

pub fn build_reverse_csr(known: &[Pos], status: &[u8]) -> (Vec<EdgeIdx>, Vec<u32>) {
    let n = known.len();
    let mut in_degree: Vec<u32> = vec![0; n];
    let mut batch: Vec<Pos> = Vec::with_capacity(BATCH_CAP + 64);
    let mut succ_buf: Vec<Pos> = Vec::new();

    for i in 0..n {
        if status[i] != STATUS_UNKNOWN { continue; }
        succ_buf.clear();
        generate_successors_canonical(known[i], &mut succ_buf);
        for &s in &succ_buf {
            batch.push(s);
        }
        if batch.len() >= BATCH_CAP {
            flush_indeg_batch(known, &mut in_degree, &mut batch);
        }
    }
    if !batch.is_empty() {
        flush_indeg_batch(known, &mut in_degree, &mut batch);
    }
    drop(batch);

    let mut rev_offset: Vec<EdgeIdx> = vec![0; n + 1];
    for i in 0..n {
        rev_offset[i + 1] = rev_offset[i] + in_degree[i] as EdgeIdx;
    }
    let total_edges = rev_offset[n] as usize;
    drop(in_degree);

    let mut rev_edges: Vec<u32> = vec![0; total_edges];
    let mut write_pos: Vec<EdgeIdx> = rev_offset[..n].to_vec();
    let mut edge_batch = EdgeBatch::new();

    for i in 0..n {
        if status[i] != STATUS_UNKNOWN { continue; }
        succ_buf.clear();
        generate_successors_canonical(known[i], &mut succ_buf);
        for &s in &succ_buf {
            edge_batch.push(s, i as u32);
        }
        if edge_batch.len() >= BATCH_CAP {
            flush_edge_batch(known, &mut rev_edges, &mut write_pos, &mut edge_batch);
        }
    }
    if edge_batch.len() > 0 {
        flush_edge_batch(known, &mut rev_edges, &mut write_pos, &mut edge_batch);
    }

    (rev_offset, rev_edges)
}

// ─── Retrograde ────────────────────────────────────────────────────────────────

pub fn retrograde(
    status: &mut [u8],
    remaining: &mut [u8],
    rev_offset: &[EdgeIdx],
    rev_edges: &[u32],
) {
    let n = status.len();
    let mut queue: Vec<u32> = Vec::new();
    for i in 0..n {
        if status[i] != STATUS_UNKNOWN {
            queue.push(i as u32);
        }
    }
    let mut qi = 0usize;
    while qi < queue.len() {
        let pi = queue[qi] as usize;
        qi += 1;
        let p_status = status[pi];
        let start = rev_offset[pi] as usize;
        let end = rev_offset[pi + 1] as usize;
        for ei in start..end {
            let pred = rev_edges[ei] as usize;
            if status[pred] != STATUS_UNKNOWN { continue; }
            if p_status == STATUS_LOSE {
                status[pred] = STATUS_WIN;
                queue.push(pred as u32);
            } else if p_status == STATUS_WIN {
                remaining[pred] -= 1;
                if remaining[pred] == 0 {
                    status[pred] = STATUS_LOSE;
                    queue.push(pred as u32);
                }
            }
        }
    }
}
