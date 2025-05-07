// tests/tnum_in_vs_xtnum_in.rs

use rand::{Rng, thread_rng};
use solana_sbpf::tnum::{Tnum, tnum_in, xtnum_in};

#[test]
fn compare_tnum_in_vs_xtnum_in_random_accuracy() {
    let mut rng = thread_rng();      // random seed
    let iterations = 500_000;        // iterations
    let mut correct = 0u32;

    for _ in 0..iterations {
        let a = Tnum::new(rng.gen::<u64>(), rng.gen::<u64>());
        let b = Tnum::new(rng.gen::<u64>(), rng.gen::<u64>());

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
