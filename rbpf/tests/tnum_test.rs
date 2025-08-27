use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::time::Instant;
use solana_sbpf::tnum::Tnum;

/// Tnum结构
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct TestTnum {
    value: u64,
    mask: u64,
}

/// 包含操作、输入和结果列表
#[derive(Debug, Serialize, Deserialize)]
struct TestCase {
    operation: String,
    input_a: TestTnum,
    input_b: TestTnum,
    results: Vec<MethodResult>,
}

/// 测试方法结果结构
#[derive(Debug, Serialize, Deserialize)]
struct MethodResult {
    method: String,
    output: TestTnum,
    avg_time_ns: f64,
}

fn run_rust_test(
    method_name: &str,
    op_fn: &dyn Fn(Tnum, Tnum) -> Tnum,
    a: Tnum,
    b: Tnum,
    iterations: usize,
) -> MethodResult {
    let mut times = Vec::with_capacity(iterations);
    let mut result = Tnum::bottom();

    for _ in 0..iterations {
        let start = Instant::now();
        result = op_fn(a, b);
        times.push(start.elapsed().as_nanos());
    }

    let output = TestTnum {
        value: result.value(),
        mask: result.mask(),
    };

    MethodResult {
        method: method_name.to_string(),
        output,
        avg_time_ns: times.iter().sum::<u128>() as f64 / iterations as f64,
    }
}

fn random_tnum() -> Tnum {
    let mut rng = thread_rng();
    let rawa: u64 = rng.gen::<u64>()%u64::MAX/2;
    let rawb: u64 = rng.gen::<u64>()%u64::MAX/2;
    Tnum::new(rawa, (rawa & rawb) ^ rawb)
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "100".to_string())
        .parse()
        .unwrap_or(100);

    let iterations: usize = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "100".to_string())
        .parse()
        .unwrap_or(100);

    println!(
        "Generating {} test cases for each operation, each repeating {} times...",
        n, iterations
    );
    let mut test_cases = Vec::new();

    let operations: Vec<(&str, Box<dyn Fn(Tnum, Tnum) -> Tnum>)> = vec![
        ("add", Box::new(|a, b| a.add(b))),
        ("sub", Box::new(|a, b| a.sub(b))),
        ("mul", Box::new(|a, b| a.mul(b))),
        ("sdiv", Box::new(|a, b| a.sdiv(b))),
        ("udiv", Box::new(|a, b| a.udiv(b))),
        ("srem", Box::new(|a, b| a.srem(b))),
        ("urem", Box::new(|a, b| a.urem(b))),
        ("and", Box::new(|a, b| a.and(&b))),
        ("or", Box::new(|a, b| a.or(&b))),
        ("xor", Box::new(|a, b| a.xor(b))),
        ("not", Box::new(|a, _| a.not())),
        ("lshr_const", Box::new(|a, b| a.lshr_const(b.value()))),
        ("shl_const", Box::new(|a, b| a.shl_const(b.value()))),
        ("ashr_const", Box::new(|a, b| a.ashr_const(b.value()))),
        ("lshr", Box::new(|a, b| a.lshr(&b))),
        ("shl", Box::new(|a, b| a.shl(&b))),
        ("eq", Box::new(|a, b| if a.contains(b) && b.contains(a) { Tnum::const_val(1) } else { Tnum::const_val(0) })),
        ("ne", Box::new(|a, b| if a.contains(b) && b.contains(a) { Tnum::const_val(0) } else { Tnum::const_val(1) })),
        ("gt", Box::new(|a, b| if a.is_nonnegative() && b.is_negative() { Tnum::const_val(1) } else { Tnum::const_val(0) })),
        ("ge", Box::new(|a, b| if !a.is_negative() || b.is_negative() { Tnum::const_val(1) } else { Tnum::const_val(0) })),
        ("lt", Box::new(|a, b| if a.is_negative() && !b.is_negative() { Tnum::const_val(1) } else { Tnum::const_val(0) })),
        ("le", Box::new(|a, b| if a.is_negative() || !b.is_negative() { Tnum::const_val(1) } else { Tnum::const_val(0) })),
    ];

    for _ in 0..n {
        let a = random_tnum();
        let b = random_tnum();

        for (op_name, op_fn) in &operations {
            let mut results = Vec::new();
            let rust_result = run_rust_test(
                &format!("Rust_{}", op_name),
                op_fn.as_ref(),
                a,
                b,
                iterations,
            );
            results.push(rust_result);

            test_cases.push(TestCase {
                operation: op_name.to_string(),
                input_a: TestTnum {
                    value: a.value(),
                    mask: a.mask(),
                },
                input_b: TestTnum {
                    value: b.value(),
                    mask: b.mask(),
                },
                results,
            });
        }
    }

    let json = serde_json::to_string_pretty(&test_cases).unwrap();
    let output_file = "./tests/build/rust_test_cases.json";
    let mut file = File::create(output_file).unwrap();
    file.write_all(json.as_bytes()).unwrap();

    println!("\nTest cases saved to: {}", output_file);
}
