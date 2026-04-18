// Disk-based BFS for large Dobutsu Shogi boards (Tromp-style external sort-merge).
//
// State on disk:
//   known.bin     sorted Vec<Pos> (all positions discovered so far)
//   frontier.bin  sorted Vec<Pos> (new positions at the current level)
//
// Per level:
//   1. Stream frontier.bin; generate successors; buffer to SORT_BUF_POS positions
//      per in-memory run, then sort/dedup each run and flush to disk.
//   2. k-way merge all runs while streaming known.bin; emit new_known.bin
//      (union) and new_frontier.bin (candidates not already in known).
//   3. Atomic rename: new_known -> known, new_frontier -> frontier. Delete runs.
//
// Env vars:
//   DISK_BFS_DIR       work directory (default ./disk_bfs_work)
//   DISK_BFS_RUN_POS   positions per in-memory sort run (default 500_000_000)

use dobutsu_count::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs::{create_dir_all, remove_file, rename, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const POS_BYTES: usize = 16;
const IO_BUF: usize = 4 * 1024 * 1024;

fn work_dir() -> PathBuf {
    std::env::var("DISK_BFS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("disk_bfs_work"))
}

fn run_buf_positions() -> usize {
    std::env::var("DISK_BFS_RUN_POS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500_000_000)
}

struct PosWriter(BufWriter<File>);
impl PosWriter {
    fn create<P: AsRef<Path>>(p: P) -> std::io::Result<Self> {
        Ok(Self(BufWriter::with_capacity(IO_BUF, File::create(p)?)))
    }
    fn write(&mut self, pos: Pos) -> std::io::Result<()> {
        self.0.write_all(&pos.to_le_bytes())
    }
    fn flush(mut self) -> std::io::Result<()> { self.0.flush() }
}

struct PosReader(BufReader<File>);
impl PosReader {
    fn open<P: AsRef<Path>>(p: P) -> std::io::Result<Self> {
        Ok(Self(BufReader::with_capacity(IO_BUF, File::open(p)?)))
    }
    fn read(&mut self) -> std::io::Result<Option<Pos>> {
        let mut buf = [0u8; POS_BYTES];
        match self.0.read_exact(&mut buf) {
            Ok(_) => Ok(Some(Pos::from_le_bytes(buf))),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(e),
        }
    }
}

fn run_path(dir: &Path, idx: usize) -> PathBuf {
    dir.join(format!("run_{:05}.bin", idx))
}

fn write_sorted_run(dir: &Path, idx: usize, buf: &mut Vec<Pos>) -> std::io::Result<()> {
    buf.sort_unstable();
    buf.dedup();
    let mut w = PosWriter::create(run_path(dir, idx))?;
    for &p in buf.iter() { w.write(p)?; }
    w.flush()?;
    buf.clear();
    Ok(())
}

// Pass 1 of a level: stream frontier, generate successors, spill sorted runs.
fn generate_runs(dir: &Path, frontier: &Path, run_cap: usize) -> std::io::Result<usize> {
    let mut r = PosReader::open(frontier)?;
    let mut buf: Vec<Pos> = Vec::with_capacity(run_cap);
    let mut succ_buf: Vec<Pos> = Vec::new();
    let mut run_idx = 0usize;

    while let Some(pos) = r.read()? {
        if is_terminal(pos) { continue; }
        succ_buf.clear();
        generate_successors_canonical(pos, &mut succ_buf);
        for &s in &succ_buf {
            buf.push(s);
            if buf.len() >= run_cap {
                write_sorted_run(dir, run_idx, &mut buf)?;
                run_idx += 1;
            }
        }
    }
    if !buf.is_empty() {
        write_sorted_run(dir, run_idx, &mut buf)?;
        run_idx += 1;
    }
    Ok(run_idx)
}

// Pass 2: k-way merge runs with known. Emits new_known = known ∪ runs,
// and new_frontier = runs \ known. Returns (new_count, new_known_total).
fn merge_level(dir: &Path, num_runs: usize) -> std::io::Result<(u64, u64)> {
    let mut readers: Vec<PosReader> = (0..num_runs)
        .map(|i| PosReader::open(run_path(dir, i)))
        .collect::<std::io::Result<_>>()?;
    let mut known = PosReader::open(dir.join("known.bin"))?;
    let mut new_frontier = PosWriter::create(dir.join("new_frontier.bin"))?;
    let mut new_known = PosWriter::create(dir.join("new_known.bin"))?;

    let mut heap: BinaryHeap<Reverse<(Pos, usize)>> = BinaryHeap::with_capacity(num_runs);
    for (i, r) in readers.iter_mut().enumerate() {
        if let Some(p) = r.read()? { heap.push(Reverse((p, i))); }
    }

    let mut cur_known = known.read()?;
    let mut last_cand: Option<Pos> = None;
    let mut new_count: u64 = 0;
    let mut total: u64 = 0;

    while let Some(Reverse((cand, run_i))) = heap.pop() {
        if let Some(next) = readers[run_i].read()? {
            heap.push(Reverse((next, run_i)));
        }
        if Some(cand) == last_cand { continue; }
        last_cand = Some(cand);

        // Copy all known entries strictly less than cand into new_known.
        while let Some(k) = cur_known {
            if k < cand {
                new_known.write(k)?;
                total += 1;
                cur_known = known.read()?;
            } else {
                break;
            }
        }

        if cur_known == Some(cand) {
            // Already known: write k once, advance known, don't emit to frontier.
            new_known.write(cand)?;
            total += 1;
            cur_known = known.read()?;
        } else {
            // New position.
            new_frontier.write(cand)?;
            new_known.write(cand)?;
            new_count += 1;
            total += 1;
        }
    }

    // Drain any remaining known.
    while let Some(k) = cur_known {
        new_known.write(k)?;
        total += 1;
        cur_known = known.read()?;
    }

    new_frontier.flush()?;
    new_known.flush()?;
    Ok((new_count, total))
}

fn cleanup_runs(dir: &Path, num_runs: usize) {
    for i in 0..num_runs {
        let _ = remove_file(run_path(dir, i));
    }
}

fn init_files(dir: &Path, init: Pos) -> std::io::Result<()> {
    let mut w = PosWriter::create(dir.join("known.bin"))?;
    w.write(init)?;
    w.flush()?;
    let mut w = PosWriter::create(dir.join("frontier.bin"))?;
    w.write(init)?;
    w.flush()?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let t0 = Instant::now();
    let dir = work_dir();
    create_dir_all(&dir)?;
    let run_cap = run_buf_positions();

    eprintln!("Disk BFS: board {}x{} ({} squares)", ROWS, COLS, NUM_SQ);
    eprintln!("  work dir: {}", dir.display());
    eprintln!(
        "  sort buffer: {} positions ({:.1} GiB)",
        run_cap,
        (run_cap as f64 * POS_BYTES as f64) / (1u64 << 30) as f64
    );

    let init = canonical(initial_position());
    init_files(&dir, init)?;
    let mut total: u64 = 1;

    for level in 1u32.. {
        let tl = Instant::now();
        let num_runs = generate_runs(&dir, &dir.join("frontier.bin"), run_cap)?;
        if num_runs == 0 {
            eprintln!("  level {:>3}: no successors (frontier all terminal); done.", level);
            break;
        }
        let (new_count, new_total) = merge_level(&dir, num_runs)?;
        cleanup_runs(&dir, num_runs);

        if new_count == 0 {
            let _ = remove_file(dir.join("new_frontier.bin"));
            let _ = remove_file(dir.join("new_known.bin"));
            eprintln!("  level {:>3}: no new positions; done.", level);
            break;
        }

        rename(dir.join("new_known.bin"), dir.join("known.bin"))?;
        rename(dir.join("new_frontier.bin"), dir.join("frontier.bin"))?;
        total = new_total;

        eprintln!(
            "  level {:>3}: runs={:>3}  +{:>13}  total={:>13}  ({:>6.1}s, cum {:>7.1}s)",
            level, num_runs, new_count, total,
            tl.elapsed().as_secs_f64(), t0.elapsed().as_secs_f64()
        );
    }

    eprintln!("Done in {:.1}s.", t0.elapsed().as_secs_f64());
    println!("Board: {}x{}", ROWS, COLS);
    println!("Reachable positions: {}", total);
    println!("Elapsed: {:.1}s", t0.elapsed().as_secs_f64());
    Ok(())
}
