use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

// 使用项目中的 wrapped_interval 实现
use solana_sbpf::wrapped_interval::WrappedRange;

/// WrappedInterval结构（用于序列化）
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct TestWrappedInterval {
    start: u64,
    end: u64,
    bitwidth: u32,
    is_bottom: bool,
}

/// 包含操作、输入和结果列表
#[derive(Debug, Serialize, Deserialize)]
struct TestCase {
    operation: String,
    input_a: TestWrappedInterval,
    input_b: TestWrappedInterval,
    results: Vec<MethodResult>,
}

/// at方法测试用例
#[derive(Debug, Serialize, Deserialize)]
struct AtTestCase {
    operation: String,
    input_a: TestWrappedInterval,
    input_value: u64,
    results: Vec<AtMethodResult>,
}

/// 比较方法测试用例（返回布尔值）
#[derive(Debug, Serialize, Deserialize)]
struct ComparisonTestCase {
    operation: String,
    input_a: TestWrappedInterval,
    input_b: TestWrappedInterval,
    results: Vec<ComparisonMethodResult>,
}

/// 单参数操作测试用例（如trunc）
#[derive(Debug, Serialize, Deserialize)]
struct UnaryTestCase {
    operation: String,
    input_a: TestWrappedInterval,
    bits_to_keep: u32,
    results: Vec<MethodResult>,
}

/// shl_const操作测试用例
#[derive(Debug, Serialize, Deserialize)]
struct ShlConstTestCase {
    operation: String,
    input_a: TestWrappedInterval,
    shift_amount: u64,
    results: Vec<MethodResult>,
}

/// 测试方法结果结构
#[derive(Debug, Serialize, Deserialize)]
struct MethodResult {
    method: String,
    output: TestWrappedInterval,
    avg_time_ns: f64,
}

/// at方法结果结构
#[derive(Debug, Serialize, Deserialize)]
struct AtMethodResult {
    method: String,
    output: bool,
    avg_time_ns: f64,
}

/// 比较方法结果结构
#[derive(Debug, Serialize, Deserialize)]
struct ComparisonMethodResult {
    method: String,
    output: bool,
    avg_time_ns: f64,
}

fn run_rust_test(
    method_name: &str,
    op_fn: &dyn Fn(&WrappedRange, &WrappedRange) -> WrappedRange,
    a: &WrappedRange,
    b: &WrappedRange,
    iterations: usize,
) -> MethodResult {
    let mut times = Vec::with_capacity(iterations);
    let mut result = WrappedRange::bottom(64);

    for _ in 0..iterations {
        let start = Instant::now();
        result = op_fn(a, b);
        times.push(start.elapsed().as_nanos());
    }

    let output = TestWrappedInterval {
        start: result.lb(),
        end: result.ub(),
        bitwidth: result.width(),
        is_bottom: result.is_bottom(),
    };

    MethodResult {
        method: method_name.to_string(),
        output,
        avg_time_ns: times.iter().sum::<u128>() as f64 / iterations as f64,
    }
}

fn run_rust_at_test(
    method_name: &str,
    a: &WrappedRange,
    value: u64,
    iterations: usize,
) -> AtMethodResult {
    let mut times = Vec::with_capacity(iterations);
    let mut result = false;

    for _ in 0..iterations {
        let start = Instant::now();
        result = a.at(value);
        times.push(start.elapsed().as_nanos());
    }

    AtMethodResult {
        method: method_name.to_string(),
        output: result,
        avg_time_ns: times.iter().sum::<u128>() as f64 / iterations as f64,
    }
}

fn run_rust_comparison_test(
    method_name: &str,
    op_fn: &dyn Fn(&WrappedRange, &WrappedRange) -> bool,
    a: &WrappedRange,
    b: &WrappedRange,
    iterations: usize,
) -> ComparisonMethodResult {
    let mut times = Vec::with_capacity(iterations);
    let mut result = false;

    for _ in 0..iterations {
        let start = Instant::now();
        result = op_fn(a, b);
        times.push(start.elapsed().as_nanos());
    }

    ComparisonMethodResult {
        method: method_name.to_string(),
        output: result,
        avg_time_ns: times.iter().sum::<u128>() as f64 / iterations as f64,
    }
}

fn run_rust_shl_const_test(
    method_name: &str,
    a: &WrappedRange,
    shift_amount: u64,
    iterations: usize,
) -> MethodResult {
    let mut times = Vec::with_capacity(iterations);
    let mut result = WrappedRange::bottom(64);

    for _ in 0..iterations {
        let start = Instant::now();
        result = a.shl_const(shift_amount);
        times.push(start.elapsed().as_nanos());
    }

    let output = TestWrappedInterval {
        start: result.lb(),
        end: result.ub(),
        bitwidth: result.width(),
        is_bottom: result.is_bottom(),
    };

    MethodResult {
        method: method_name.to_string(),
        output,
        avg_time_ns: times.iter().sum::<u128>() as f64 / iterations as f64,
    }
}

fn run_rust_lshr_const_test(
    method_name: &str,
    a: &WrappedRange,
    shift_amount: u64,
    iterations: usize,
) -> MethodResult {
    let mut times = Vec::with_capacity(iterations);
    let mut result = WrappedRange::bottom(64);

    for _ in 0..iterations {
        let start = Instant::now();
        result = a.lshr_const(shift_amount);
        times.push(start.elapsed().as_nanos());
    }

    let output = TestWrappedInterval {
        start: result.lb(),
        end: result.ub(),
        bitwidth: result.width(),
        is_bottom: result.is_bottom(),
    };

    MethodResult {
        method: method_name.to_string(),
        output,
        avg_time_ns: times.iter().sum::<u128>() as f64 / iterations as f64,
    }
}

fn random_wrapped_interval_no_zero() -> WrappedRange {
    let mut rng = thread_rng();
    let bitwidth = 64u32;
    let max_val = 1024;
    // Generate a start value greater than 0
    let start = rng.gen_range(1..max_val);
    // Generate an end value greater than start, ensuring no wrap-around that includes 0
    let end = rng.gen_range(start..max_val);
    WrappedRange::new_bounds(start, end, bitwidth)
}

fn random_wrapped_interval() -> WrappedRange {
    let mut rng = thread_rng();
    let bitwidth = 64u32;
    // let max_val = if bitwidth == 64 { u64::MAX } else { (1u64 << bitwidth) - 1 };
    let max_val = 1024;

    let start = rng.gen::<u64>() % max_val;
    let end = rng.gen::<u64>() % max_val;

    WrappedRange::new_bounds(start, end, bitwidth)
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "1".to_string()) // 改成1进行调试
        .parse()
        .unwrap_or(1);

    let iterations: usize = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "1".to_string()) // 改成1进行调试
        .parse()
        .unwrap_or(1);

    println!(
        "Generating {} test cases for each wrapped-interval operation, each repeating {} times...",
        n, iterations
    );
    let mut test_cases = Vec::new();
    let mut at_test_cases = Vec::new();
    let mut comparison_test_cases = Vec::new();
    let mut unary_test_cases = Vec::new();
    let mut shl_const_test_cases = Vec::new();
    let mut lshr_const_test_cases = Vec::new();

    let operations: Vec<(
        &str,
        Box<dyn Fn(&WrappedRange, &WrappedRange) -> WrappedRange>,
    )> = vec![
        ("add", Box::new(|a, b| a.add(b))),
        ("sub", Box::new(|a, b| a.sub(b))),
        ("unsigned_mul", Box::new(|a, b| a.unsigned_mul(b))),
        ("signed_mul", Box::new(|a, b| a.signed_mul(b))),
        ("mul", Box::new(|a, b| a.mul(b))),
        ("signed_div", Box::new(|a, b| a.signed_div(b))),
        ("unsigned_div", Box::new(|a, b| a.unsigned_div(b))),
        ("udiv", Box::new(|a, b| a.udiv(b))),
        ("sdiv", Box::new(|a, b| a.sdiv(b))),
        // ("urem", Box::new(|a, b| a.urem(b))),
        // ("srem", Box::new(|a, b| a.srem(b))),
        ("and", Box::new(|a, b| a.and(b))),
        ("or", Box::new(|a, b| a.or(b))),
        // ("xor", Box::new(|a, b| a.xor(b))),
        ("shl", Box::new(|a, b| a.shl(b))),
        // ("lshr", Box::new(|a, b| a.lshr(b))),
        // ("ashr", Box::new(|a, b| a.ashr(b))),
        // ("meet", Box::new(|a, b| a.meet(b))),
        // ("join", Box::new(|a, b| a.generalized_join(b))),
        // ("widening", Box::new(|a, b| a.widening_join(b))),
        // ("narrowing", Box::new(|a, b| a.narrowing(b))),
    ];

    let comparison_operations: Vec<(&str, Box<dyn Fn(&WrappedRange, &WrappedRange) -> bool>)> =
        vec![("less_or_equal", Box::new(|a, b| a.less_or_equal(b)))];

    for _ in 0..n {
        let a = random_wrapped_interval();
        let b = random_wrapped_interval();

        for (op_name, op_fn) in &operations {
            let mut results = Vec::new();

            // Create a temporary variable `owned_divisor` for the special case.
            // `divisor_ref` will then be a reference to either `b` or `owned_divisor`.
            let owned_divisor;
            let divisor_ref = if *op_name == "signed_div" || *op_name == "unsigned_div" {
                owned_divisor = random_wrapped_interval_no_zero();
                &owned_divisor
            } else {
                &b
            };

            let rust_result = run_rust_test(
                &format!("Rust_{}", op_name),
                op_fn.as_ref(),
                &a,
                divisor_ref,
                iterations,
            );
            results.push(rust_result);

            test_cases.push(TestCase {
                operation: op_name.to_string(),
                input_a: TestWrappedInterval {
                    start: a.lb(),
                    end: a.ub(),
                    bitwidth: a.width(),
                    is_bottom: a.is_bottom(),
                },
                input_b: TestWrappedInterval {
                    start: divisor_ref.lb(),
                    end: divisor_ref.ub(),
                    bitwidth: divisor_ref.width(),
                    is_bottom: divisor_ref.is_bottom(),
                },
                results,
            });
        }

        // 添加比较方法测试
        for (comp_name, comp_fn) in &comparison_operations {
            let mut comp_results = Vec::new();
            let rust_comp_result = run_rust_comparison_test(
                &format!("Rust_{}", comp_name),
                comp_fn.as_ref(),
                &a,
                &b,
                iterations,
            );
            comp_results.push(rust_comp_result);

            comparison_test_cases.push(ComparisonTestCase {
                operation: comp_name.to_string(),
                input_a: TestWrappedInterval {
                    start: a.lb(),
                    end: a.ub(),
                    bitwidth: a.width(),
                    is_bottom: a.is_bottom(),
                },
                input_b: TestWrappedInterval {
                    start: b.lb(),
                    end: b.ub(),
                    bitwidth: b.width(),
                    is_bottom: b.is_bottom(),
                },
                results: comp_results,
            });
        }

        // 添加 at 方法测试
        let test_value = b.lb(); // 使用 b 的下界作为测试值
        let mut at_results = Vec::new();
        let rust_at_result = run_rust_at_test("Rust_at", &a, test_value, iterations);
        at_results.push(rust_at_result);

        at_test_cases.push(AtTestCase {
            operation: "at".to_string(),
            input_a: TestWrappedInterval {
                start: a.lb(),
                end: a.ub(),
                bitwidth: a.width(),
                is_bottom: a.is_bottom(),
            },
            input_value: test_value,
            results: at_results,
        });
    }

    // 测试trunc操作
    for _ in 0..n {
        let a = random_wrapped_interval();
        let bits_to_keep = thread_rng().gen_range(1..=a.width());
        
        let mut results = Vec::new();
        
        // 测试Rust实现
        let rust_result = a.trunc(bits_to_keep);
        results.push(MethodResult {
            method: "rust".to_string(),
            output: TestWrappedInterval {
                start: rust_result.lb(),
                end: rust_result.ub(),
                bitwidth: rust_result.width(),
                is_bottom: rust_result.is_bottom(),
            },
            avg_time_ns: 0.0, // trunc操作通常很快，设为0
        });
        
        unary_test_cases.push(UnaryTestCase {
            operation: "trunc".to_string(),
            input_a: TestWrappedInterval {
                start: a.lb(),
                end: a.ub(),
                bitwidth: a.width(),
                is_bottom: a.is_bottom(),
            },
            bits_to_keep,
            results,
        });
    }

    // 测试shl_const操作
    for _ in 0..n {
        let a = random_wrapped_interval();
        let shift_amount = thread_rng().gen_range(1..=a.width() as u64);

        let mut results = Vec::new();
        // 测试Rust实现
        let rust_result = run_rust_shl_const_test(
            "Rust_shl_const",
            &a,
            shift_amount,
            iterations,
        );
        results.push(rust_result);

        shl_const_test_cases.push(ShlConstTestCase {
            operation: "shl_const".to_string(),
            input_a: TestWrappedInterval {
                start: a.lb(),
                end: a.ub(),
                bitwidth: a.width(),
                is_bottom: a.is_bottom(),
            },
            shift_amount,
            results,
        });
    }

    // 测试lshr_const操作
    for _ in 0..n {
        let a = random_wrapped_interval();
        let shift_amount = thread_rng().gen_range(1..=a.width() as u64);

        let mut results = Vec::new();
        // 测试Rust实现
        let rust_result = run_rust_lshr_const_test(
            "Rust_lshr_const",
            &a,
            shift_amount,
            iterations,
        );
        results.push(rust_result);

        lshr_const_test_cases.push(ShlConstTestCase {
            operation: "lshr_const".to_string(),
            input_a: TestWrappedInterval {
                start: a.lb(),
                end: a.ub(),
                bitwidth: a.width(),
                is_bottom: a.is_bottom(),
            },
            shift_amount,
            results,
        });
    }

    // 创建包含五种测试类型的JSON输出
    #[derive(Serialize)]
    struct AllTestCases {
        binary_operations: Vec<TestCase>,
        at_operations: Vec<AtTestCase>,
        comparison_operations: Vec<ComparisonTestCase>,
        unary_operations: Vec<UnaryTestCase>,
        shl_const_operations: Vec<ShlConstTestCase>,
        lshr_const_operations: Vec<ShlConstTestCase>,
    }

    let all_tests = AllTestCases {
        binary_operations: test_cases,
        at_operations: at_test_cases,
        comparison_operations: comparison_test_cases,
        unary_operations: unary_test_cases,
        shl_const_operations: shl_const_test_cases,
        lshr_const_operations: lshr_const_test_cases,
    };

    let json = serde_json::to_string_pretty(&all_tests).unwrap();
    let output_file = "./tests/build/rust_wrapped_interval_cases.json";

    // 确保输出目录存在
    std::fs::create_dir_all("./tests/build").unwrap();

    let mut file = File::create(output_file).unwrap();
    file.write_all(json.as_bytes()).unwrap();

    println!("\nWrapped interval test cases saved to: {}", output_file);
}
