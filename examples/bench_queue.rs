use crossbeam_queue::ArrayQueue;
use nethuns_rs::api::{BufferDesc, Token};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const ITERS: u64 = 40_000_000;
const Q_SIZE: usize = 65536;

fn new_token(i: usize) -> Token {
    Token::new(BufferDesc::from(i), 0, 100)
}

fn print_res(name: &str, iters: u64, duration: Duration) {
    let nanos = duration.as_nanos() as f64;
    let seconds = nanos / 1_000_000_000.0;
    let mpps = (iters as f64 / 1_000_000.0) / seconds;
    println!("{:<25} : {:.2} M/s", name, mpps);
}

// -----------------------------------------------------------------------------
// Flume
// -----------------------------------------------------------------------------

fn bench_flume_sp() {
    let (tx, rx) = flume::bounded(Q_SIZE);

    let t = thread::spawn(move || {
        for i in 0..ITERS {
            tx.send(new_token(i as usize)).unwrap();
        }
    });

    let start = Instant::now();
    let mut count = 0;
    while count < ITERS {
        if let Ok(_) = rx.recv() {
            count += 1;
        }
    }
    let duration = start.elapsed();

    t.join().unwrap();
    print_res("flume (1->1)", ITERS, duration);
}

fn bench_flume_mp(threads: usize) {
    let (tx, rx) = flume::bounded(Q_SIZE);
    let mut handles = Vec::new();
    let iter_per_thread = ITERS / threads as u64;
    let total_iters = iter_per_thread * threads as u64;

    for _ in 0..threads {
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            for i in 0..iter_per_thread {
                tx.send(new_token(i as usize)).unwrap();
            }
        }));
    }

    let start = Instant::now();
    let mut count = 0;
    while count < total_iters {
        if let Ok(_) = rx.recv() {
            count += 1;
        }
    }
    let duration = start.elapsed();

    for h in handles {
        h.join().unwrap();
    }
    print_res(&format!("flume ({}->1)", threads), total_iters, duration);
}

// -----------------------------------------------------------------------------
// Crossbeam ArrayQueue
// -----------------------------------------------------------------------------

fn bench_crossbeam_sp() {
    let q = Arc::new(ArrayQueue::new(Q_SIZE));
    let q_prod = q.clone();

    let t = thread::spawn(move || {
        for i in 0..ITERS {
            loop {
                if let Ok(_) = q_prod.push(new_token(i as usize)) {
                    break;
                }
                // Backoff or yield?
                thread::yield_now();
            }
        }
    });

    let start = Instant::now();
    let mut count = 0;
    while count < ITERS {
        if let Some(_) = q.pop() {
            count += 1;
        } else {
            thread::yield_now();
        }
    }
    let duration = start.elapsed();

    t.join().unwrap();
    print_res("crossbeam (1->1)", ITERS, duration);
}

fn bench_crossbeam_mp(threads: usize) {
    let q = Arc::new(ArrayQueue::new(Q_SIZE));
    let mut handles = Vec::new();
    let iter_per_thread = ITERS / threads as u64;
    let total_iters = iter_per_thread * threads as u64;

    for _ in 0..threads {
        let q_prod = q.clone();
        handles.push(thread::spawn(move || {
            for i in 0..iter_per_thread {
                loop {
                    if let Ok(_) = q_prod.push(new_token(i as usize)) {
                        break;
                    }
                    thread::yield_now();
                }
            }
        }));
    }

    let start = Instant::now();
    let mut count = 0;
    while count < total_iters {
        if let Some(_) = q.pop() {
            count += 1;
        } else {
            thread::yield_now();
        }
    }
    let duration = start.elapsed();

    for h in handles {
        h.join().unwrap();
    }
    print_res(
        &format!("crossbeam ({}->1)", threads),
        total_iters,
        duration,
    );
}

// -----------------------------------------------------------------------------
// Nethuns MPSC
// -----------------------------------------------------------------------------

fn bench_mpsc_sp() {
    // Note: mpsc::channel<T> but the implementation enforces usize transport.
    // We pass Token::idx (BufferDesc) which is usize.
    let (mut prod, mut cons) = mpsc::channel::<Token>(Q_SIZE);

    let t = thread::spawn(move || {
        for i in 0..ITERS {
            let token = new_token(i as usize);
            prod.push(usize::from(token.buffer_desc()));
        }
    });

    let start = Instant::now();
    let mut count = 0;
    while count < ITERS {
        if let Some(_) = cons.pop() {
            count += 1;
        } else {
            thread::yield_now();
        }
    }
    let duration = start.elapsed();

    t.join().unwrap();
    print_res("nethuns-mpsc (1->1)", ITERS, duration);
}

fn bench_mpsc_mp(threads: usize) {
    let (prod, mut cons) = mpsc::channel::<Token>(Q_SIZE);
    let mut handles = Vec::new();
    let iter_per_thread = ITERS / threads as u64;
    let total_iters = iter_per_thread * threads as u64;

    for _ in 0..threads {
        let mut prod = prod.clone();
        handles.push(thread::spawn(move || {
            for i in 0..iter_per_thread {
                let token = new_token(i as usize);
                prod.push(usize::from(token.buffer_desc()));
            }
        }));
    }

    let start = Instant::now();
    let mut count = 0;
    while count < total_iters {
        if let Some(_) = cons.pop() {
            count += 1;
        } else {
            thread::yield_now();
        }
    }
    let duration = start.elapsed();

    for h in handles {
        h.join().unwrap();
    }
    print_res(
        &format!("nethuns-mpsc ({}->1)", threads),
        total_iters,
        duration,
    );
}

// -----------------------------------------------------------------------------
// std::sync::mpsc
// -----------------------------------------------------------------------------

fn bench_std_mpsc_sp() {
    let (tx, rx) = std::sync::mpsc::sync_channel(Q_SIZE);

    let t = thread::spawn(move || {
        for i in 0..ITERS {
            tx.send(new_token(i as usize)).unwrap();
        }
    });

    let start = Instant::now();
    let mut count = 0;
    while count < ITERS {
        if let Ok(_) = rx.recv() {
            count += 1;
        }
    }
    let duration = start.elapsed();

    t.join().unwrap();
    print_res("std::sync::mpsc (1->1)", ITERS, duration);
}

fn bench_std_mpsc_mp(threads: usize) {
    let (tx, rx) = std::sync::mpsc::sync_channel(Q_SIZE);
    let mut handles = Vec::new();
    let iter_per_thread = ITERS / threads as u64;
    let total_iters = iter_per_thread * threads as u64;

    for _ in 0..threads {
        let tx = tx.clone();
        handles.push(thread::spawn(move || {
            for i in 0..iter_per_thread {
                tx.send(new_token(i as usize)).unwrap();
            }
        }));
    }

    let start = Instant::now();
    let mut count = 0;
    while count < total_iters {
        if let Ok(_) = rx.recv() {
            count += 1;
        }
    }
    let duration = start.elapsed();

    for h in handles {
        h.join().unwrap();
    }
    print_res(
        &format!("std::sync::mpsc ({}->1)", threads),
        total_iters,
        duration,
    );
}

fn main() {
    println!("Benchmarking queues with {} iters...", ITERS);

    bench_flume_sp();
    bench_crossbeam_sp();
    bench_mpsc_sp();
    bench_std_mpsc_sp();

    println!("---");

    bench_flume_mp(4);
    bench_crossbeam_mp(4);
    bench_mpsc_mp(4);
    bench_std_mpsc_mp(4);
}
