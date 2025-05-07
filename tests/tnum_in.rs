// tests/tnum_in_vs_xtnum_in.rs

use rand::{Rng, thread_rng};
use solana_sbpf::tnum::{Tnum, tnum_in, xtnum_in};

fn random_tnum() -> Tnum {
    let mut rng = thread_rng(); //random seed
    let rawa: u64 = rng.gen();
    let rawb: u64 = rng.gen();
    Tnum::new(rawa, (rawa & rawb) ^ rawb)
}

#[test]
fn compare_tnum_in_vs_xtnum_in_random_accuracy() {
    let iterations = 500_000;        // iterations
    let mut correct = 0u32;

    for _ in 0..iterations {
        let a: Tnum = random_tnum();
        let b: Tnum = random_tnum();

        if tnum_in(a, b) == xtnum_in(a, b) {
            correct += 1;
        }
    }

    // accuracy
    let accuracy = correct as f64 / iterations as f64 * 100.0;
    println!(
        "tnum_in vs xtnum_in accuracy:{}/{} = {:.2}%",
        correct, iterations, accuracy
    );
}
