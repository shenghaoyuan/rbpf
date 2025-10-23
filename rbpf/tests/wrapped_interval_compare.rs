// 文件名: tests/wrapped_interval_compare.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;

// 统计信息结构体
#[derive(Clone)]
struct MethodStats {
    method: String,
    equal: u32,
    less_than: u32,
    more_than: u32,
    not_equal: u32,
    total_count: u32,
    total_time: f64,
}

impl MethodStats {
    fn new(method: &str) -> Self {
        MethodStats {
            method: method.to_string(),
            equal: 0,
            less_than: 0,
            more_than: 0,
            not_equal: 0,
            total_count: 0,
            total_time: 0.0,
        }
    }

    fn avg_time(&self) -> f64 {
        if self.total_count > 0 {
            self.total_time / self.total_count as f64
        } else {
            0.0
        }
    }
}

// 不一致结果结构体
#[derive(Serialize)]
struct Inconsistency {
    case_number: u32,
    operation: String,
    input_a: WrappedIntervalValue,
    input_b: Option<WrappedIntervalValue>,
    input_value: Option<u64>,
    bits_to_keep: Option<u32>,
    baseline_output: Option<WrappedIntervalValue>,
    baseline_bool_output: Option<bool>,
    rust_output: Option<WrappedIntervalValue>,
    rust_bool_output: Option<bool>,
    method: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WrappedIntervalValue {
    start: u64,
    end: u64,
    bitwidth: u32,
    is_bottom: bool,
}

#[derive(Deserialize)]
struct TestCase {
    operation: String,
    input_a: WrappedIntervalValue,
    input_b: WrappedIntervalValue,
    results: Vec<MethodResult>,
}

#[derive(Deserialize)]
struct AtTestCase {
    operation: String,
    input_a: WrappedIntervalValue,
    input_value: u64,
    results: Vec<AtMethodResult>,
}

#[derive(Deserialize)]
struct ComparisonTestCase {
    operation: String,
    input_a: WrappedIntervalValue,
    input_b: WrappedIntervalValue,
    results: Vec<AtMethodResult>,
}

#[derive(Deserialize)]
struct TestData {
    binary_operations: Vec<TestCase>,
    at_operations: Vec<AtTestCase>,
    comparison_operations: Option<Vec<ComparisonTestCase>>,
    unary_operations: Option<Vec<UnaryTestCase>>,
    shl_const_operations: Option<Vec<ShlConstTestCase>>,
    lshr_const_operations: Option<Vec<ShlConstTestCase>>,
}

#[derive(Clone, Deserialize)]
struct MethodResult {
    method: String,
    output: WrappedIntervalValue,
    avg_time_ns: f64,
}

#[derive(Deserialize)]
struct AtMethodResult {
    method: String,
    output: bool,
    avg_time_ns: f64,
}

// 单参数操作测试用例（如trunc）
#[derive(Deserialize)]
struct UnaryTestCase {
    operation: String,
    input_a: WrappedIntervalValue,
    bits_to_keep: u32,
    results: Vec<MethodResult>,
}

// shl_const操作测试用例
#[derive(Deserialize)]
struct ShlConstTestCase {
    operation: String,
    input_a: WrappedIntervalValue,
    shift_amount: u64,
    results: Vec<MethodResult>,
}

// 比较两个wrapped interval的关系
fn compare_intervals(a: &WrappedIntervalValue, b: &WrappedIntervalValue) -> String {
    if intervals_equal(a, b) {
        return "equal".to_string();
    }
    
    // 直接返回不相等，不进行包含关系判断
    "not_equal".to_string()
}

// 比较两个wrapped interval是否相等
fn intervals_equal(a: &WrappedIntervalValue, b: &WrappedIntervalValue) -> bool {
    // 使用Rust的WrappedRange::equal函数进行语义比较
    use solana_sbpf::wrapped_interval::WrappedRange;
    
    // 创建WrappedRange对象进行比较
    let range_a = WrappedRange::new_bounds(a.start, a.end, a.bitwidth);
    let range_b = WrappedRange::new_bounds(b.start, b.end, b.bitwidth);
    
    // 使用equal函数进行语义比较
    range_a.equal(&range_b)
}

impl std::fmt::Display for WrappedIntervalValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_bottom {
            write!(f, "⊥ (bottom)")
        } else {
            write!(f, "[{}, {}] ({}bit)", self.start, self.end, self.bitwidth)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <cpp_test_results.json> [rust_test_results.json]", args[0]);
        return Err("Insufficient arguments".into());
    }

    let cpp_file = &args[1];
    let rust_file = if args.len() > 2 { Some(&args[2]) } else { None };
    
    let cpp_json_data = fs::read_to_string(cpp_file)?;
    
    let mut test_data: TestData = match serde_json::from_str(&cpp_json_data) {
        Ok(data) => data,
        Err(_) => {
            // 尝试旧格式
            let test_cases: Vec<TestCase> = serde_json::from_str(&cpp_json_data)?;
            TestData {
                binary_operations: test_cases,
                at_operations: vec![],
                comparison_operations: None,
                unary_operations: None,
                shl_const_operations: None,
                lshr_const_operations: None,
            }
        }
    };
    
    // 如果提供了Rust结果文件，合并Rust的unary_operations和shl_const_operations
    if let Some(rust_file) = rust_file {
        let rust_json_data = fs::read_to_string(rust_file)?;
        if let Ok(rust_data) = serde_json::from_str::<TestData>(&rust_json_data) {
            if let Some(rust_unary_ops) = rust_data.unary_operations {
                // 合并Rust的unary_operations到现有的test_data中
                if let Some(cpp_unary_ops) = &mut test_data.unary_operations {
                    // 合并每个测试用例的结果
                    for (i, rust_case) in rust_unary_ops.iter().enumerate() {
                        if i < cpp_unary_ops.len() {
                            cpp_unary_ops[i].results.extend(rust_case.results.clone());
                        }
                    }
                } else {
                    test_data.unary_operations = Some(rust_unary_ops);
                }
            }
            
            if let Some(rust_shl_const_ops) = rust_data.shl_const_operations {
                // 合并Rust的shl_const_operations到现有的test_data中
                if let Some(cpp_shl_const_ops) = &mut test_data.shl_const_operations {
                    // 合并每个测试用例的结果
                    for (i, rust_case) in rust_shl_const_ops.iter().enumerate() {
                        if i < cpp_shl_const_ops.len() {
                            cpp_shl_const_ops[i].results.extend(rust_case.results.clone());
                        }
                    }
                } else {
                    test_data.shl_const_operations = Some(rust_shl_const_ops);
                }
            }
        }
    }

    let total_binary = test_data.binary_operations.len();
    let total_at = test_data.at_operations.len();
    let total_comparison = test_data.comparison_operations.as_ref().map_or(0, |ops| ops.len());
    let total_unary = test_data.unary_operations.as_ref().map_or(0, |ops| ops.len());
    let total_shl_const = test_data.shl_const_operations.as_ref().map_or(0, |ops| ops.len());
    let total_lshr_const = test_data.lshr_const_operations.as_ref().map_or(0, |ops| ops.len());
    println!("Analyzing {} binary operation test cases, {} at operation test cases, {} comparison operation test cases, {} unary operation test cases, {} shl_const operation test cases, and {} lshr_const operation test cases...", total_binary, total_at, total_comparison, total_unary, total_shl_const, total_lshr_const);

    let mut stats: HashMap<String, MethodStats> = HashMap::new();
    let mut inconsistencies: Vec<Inconsistency> = Vec::new();

    // 处理二元操作
    for (i, test_case) in test_data.binary_operations.iter().enumerate() {
        let baseline_method_name = format!("CPP_{}", test_case.operation);
        let baseline_result = test_case.results.iter().find(|r| r.method == baseline_method_name);

        if let Some(baseline) = baseline_result {
            // Update stats for the baseline method
            let s = stats.entry(baseline.method.clone()).or_insert_with(|| MethodStats::new(&baseline.method));
            s.total_count += 1;
            s.total_time += baseline.avg_time_ns;
            s.equal += 1;

            for result in &test_case.results {
                if result.method.starts_with("CPP_") {
                    continue;
                }
                
                let s = stats.entry(result.method.clone()).or_insert_with(|| MethodStats::new(&result.method));
                s.total_count += 1;
                s.total_time += result.avg_time_ns;

                let comparison = compare_intervals(&baseline.output, &result.output);

                match comparison.as_str() {
                    "equal" => s.equal += 1,
                    "less_than" => s.less_than += 1,
                    "more_than" => s.more_than += 1,
                    _ => {
                        s.not_equal += 1;
                        inconsistencies.push(Inconsistency {
                            case_number: (i + 1) as u32,
                            operation: test_case.operation.clone(),
                            input_a: test_case.input_a.clone(),
                            input_b: Some(test_case.input_b.clone()),
                            input_value: None,
                            bits_to_keep: None,
                            baseline_output: Some(baseline.output.clone()),
                            baseline_bool_output: None,
                            rust_output: Some(result.output.clone()),
                            rust_bool_output: None,
                            method: result.method.clone(),
                        });
                    }
                }
            }
        }
    }

    // 处理at操作
    for (i, test_case) in test_data.at_operations.iter().enumerate() {
        let baseline_method_name = "CPP_at";
        let baseline_result = test_case.results.iter().find(|r| r.method == baseline_method_name);

        if let Some(baseline) = baseline_result {
            // Update stats for the baseline method
            let s = stats.entry(baseline.method.clone()).or_insert_with(|| MethodStats::new(&baseline.method));
            s.total_count += 1;
            s.total_time += baseline.avg_time_ns;
            s.equal += 1;

            for result in &test_case.results {
                if result.method.starts_with("CPP_") {
                    continue;
                }
                
                let s = stats.entry(result.method.clone()).or_insert_with(|| MethodStats::new(&result.method));
                s.total_count += 1;
                s.total_time += result.avg_time_ns;

                if baseline.output == result.output {
                    s.equal += 1;
                } else {
                    s.not_equal += 1;
                    inconsistencies.push(Inconsistency {
                        case_number: (total_binary + i + 1) as u32,
                        operation: test_case.operation.clone(),
                        input_a: test_case.input_a.clone(),
                        input_b: None,
                        input_value: Some(test_case.input_value),
                        bits_to_keep: None,
                        baseline_output: None,
                        baseline_bool_output: Some(baseline.output),
                        rust_output: None,
                        rust_bool_output: Some(result.output),
                        method: result.method.clone(),
                    });
                }
            }
        }
    }

    // 处理比较操作
    if let Some(comparison_ops) = &test_data.comparison_operations {
        for (i, test_case) in comparison_ops.iter().enumerate() {
            let baseline_method_name = format!("CPP_{}", test_case.operation);
            let baseline_result = test_case.results.iter().find(|r| r.method == baseline_method_name);

            if let Some(baseline) = baseline_result {
                // Update stats for the baseline method
                let s = stats.entry(baseline.method.clone()).or_insert_with(|| MethodStats::new(&baseline.method));
                s.total_count += 1;
                s.total_time += baseline.avg_time_ns;
                s.equal += 1;

                for result in &test_case.results {
                    if result.method.starts_with("CPP_") {
                        continue;
                    }
                    
                    let s = stats.entry(result.method.clone()).or_insert_with(|| MethodStats::new(&result.method));
                    s.total_count += 1;
                    s.total_time += result.avg_time_ns;

                    if baseline.output == result.output {
                        s.equal += 1;
                    } else {
                        s.not_equal += 1;
                        inconsistencies.push(Inconsistency {
                            case_number: (total_binary + total_at + i + 1) as u32,
                            operation: test_case.operation.clone(),
                            input_a: test_case.input_a.clone(),
                            input_b: Some(test_case.input_b.clone()),
                            input_value: None,
                            bits_to_keep: None,
                            baseline_output: None,
                            baseline_bool_output: Some(baseline.output),
                            rust_output: None,
                            rust_bool_output: Some(result.output),
                            method: result.method.clone(),
                        });
                    }
                }
            }
        }
    }

    // 处理单参数操作（如trunc）
    if let Some(unary_ops) = &test_data.unary_operations {
        for (i, test_case) in unary_ops.iter().enumerate() {
            let baseline_method_name = format!("CPP_{}", test_case.operation);
            let baseline_result = test_case.results.iter().find(|r| r.method == baseline_method_name);

            if let Some(baseline) = baseline_result {
                // Update stats for the baseline method
                let s = stats.entry(baseline.method.clone()).or_insert_with(|| MethodStats::new(&baseline.method));
                s.total_count += 1;
                s.total_time += baseline.avg_time_ns;
                s.equal += 1;

                for result in &test_case.results {
                    if result.method.starts_with("CPP_") {
                        continue;
                    }
                    
                    // 规范化Rust方法名称
                    let normalized_method = if result.method == "rust" {
                        format!("Rust_{}", test_case.operation)
                    } else {
                        result.method.clone()
                    };
                    
                    let s = stats.entry(normalized_method.clone()).or_insert_with(|| MethodStats::new(&normalized_method));
                    s.total_count += 1;
                    s.total_time += result.avg_time_ns;

                    let comparison_result = compare_intervals(&baseline.output, &result.output);
                    match comparison_result.as_str() {
                        "equal" => s.equal += 1,
                        "not_equal" => s.not_equal += 1,
                        _ => s.not_equal += 1,
                    }

                    if comparison_result != "equal" {
                        inconsistencies.push(Inconsistency {
                            case_number: (total_binary + total_at + total_comparison + i + 1) as u32,
                            operation: test_case.operation.clone(),
                            input_a: test_case.input_a.clone(),
                            input_b: None,
                            input_value: None,
                            bits_to_keep: Some(test_case.bits_to_keep),
                            baseline_output: Some(baseline.output.clone()),
                            baseline_bool_output: None,
                            rust_output: Some(result.output.clone()),
                            rust_bool_output: None,
                            method: normalized_method.clone(),
                        });
                    }
                }
            }
        }
    }

    // 处理shl_const操作
    if let Some(shl_const_ops) = &test_data.shl_const_operations {
        for (i, test_case) in shl_const_ops.iter().enumerate() {
            let baseline_method_name = format!("CPP_{}", test_case.operation);
            let baseline_result = test_case.results.iter().find(|r| r.method == baseline_method_name);

            if let Some(baseline) = baseline_result {
                let s = stats.entry(baseline.method.clone()).or_insert_with(|| MethodStats::new(&baseline.method));
                s.total_count += 1;
                s.total_time += baseline.avg_time_ns;
                s.equal += 1;

                for result in &test_case.results {
                    if result.method.starts_with("CPP_") {
                        continue;
                    }
                    let normalized_method = if result.method == "rust" {
                        format!("Rust_{}", test_case.operation)
                    } else {
                        result.method.clone()
                    };
                    let s = stats.entry(normalized_method.clone()).or_insert_with(|| MethodStats::new(&normalized_method));
                    s.total_count += 1;
                    s.total_time += result.avg_time_ns;
                    let comparison_result = compare_intervals(&baseline.output, &result.output);
                    match comparison_result.as_str() {
                        "equal" => s.equal += 1,
                        "not_equal" => s.not_equal += 1,
                        _ => s.not_equal += 1,
                    }
                    if comparison_result != "equal" {
                        inconsistencies.push(Inconsistency {
                            case_number: (total_binary + total_at + total_comparison + total_unary + i + 1) as u32,
                            operation: test_case.operation.clone(),
                            input_a: test_case.input_a.clone(),
                            input_b: None,
                            input_value: Some(test_case.shift_amount),
                            bits_to_keep: None,
                            baseline_output: Some(baseline.output.clone()),
                            baseline_bool_output: None,
                            rust_output: Some(result.output.clone()),
                            rust_bool_output: None,
                            method: normalized_method.clone(),
                        });
                    }
                }
            }
        }
    }

    // 处理lshr_const操作
    if let Some(lshr_const_ops) = &test_data.lshr_const_operations {
        for (i, test_case) in lshr_const_ops.iter().enumerate() {
            let baseline_method_name = format!("CPP_{}", test_case.operation);
            let baseline_result = test_case.results.iter().find(|r| r.method == baseline_method_name);

            if let Some(baseline) = baseline_result {
                let s = stats.entry(baseline.method.clone()).or_insert_with(|| MethodStats::new(&baseline.method));
                s.total_count += 1;
                s.total_time += baseline.avg_time_ns;
                s.equal += 1;

                for result in &test_case.results {
                    if result.method.starts_with("CPP_") {
                        continue;
                    }
                    let normalized_method = if result.method == "rust" {
                        format!("Rust_{}", test_case.operation)
                    } else {
                        result.method.clone()
                    };
                    let s = stats.entry(normalized_method.clone()).or_insert_with(|| MethodStats::new(&normalized_method));
                    s.total_count += 1;
                    s.total_time += result.avg_time_ns;
                    let comparison_result = compare_intervals(&baseline.output, &result.output);
                    match comparison_result.as_str() {
                        "equal" => s.equal += 1,
                        "not_equal" => s.not_equal += 1,
                        _ => s.not_equal += 1,
                    }
                    if comparison_result != "equal" {
                        inconsistencies.push(Inconsistency {
                            case_number: (total_binary + total_at + total_comparison + total_unary + total_shl_const + i + 1) as u32,
                            operation: test_case.operation.clone(),
                            input_a: test_case.input_a.clone(),
                            input_b: None,
                            input_value: Some(test_case.shift_amount),
                            bits_to_keep: None,
                            baseline_output: Some(baseline.output.clone()),
                            baseline_bool_output: None,
                            rust_output: Some(result.output.clone()),
                            rust_bool_output: None,
                            method: normalized_method.clone(),
                        });
                    }
                }
            }
        }
    }

    println!("\n");
    println!("{:<24} {:<18} {:<10} {:<10} {:<10} {:<10}", "Method", "Avg Time (ns)", "Equal (%)", "Less (%)", "More (%)", "Other (%)");
    println!("{}", "-".repeat(82));

    let mut sorted_stats: Vec<_> = stats.values().cloned().collect();
    sorted_stats.sort_by_key(|s| s.method.clone());

    for stat in &sorted_stats {
        if stat.total_count > 0 {
            let equal_pct = stat.equal as f64 / stat.total_count as f64 * 100.0;
            let less_pct = stat.less_than as f64 / stat.total_count as f64 * 100.0;
            let more_pct = stat.more_than as f64 / stat.total_count as f64 * 100.0;
            let not_equal_pct = stat.not_equal as f64 / stat.total_count as f64 * 100.0;
            println!(
                "{:<24} {:<18.1} {:<10.1} {:<10.1} {:<10.1} {:<10.1}",
                stat.method, stat.avg_time(), equal_pct, less_pct, more_pct, not_equal_pct
            );
        }
    }

    if !inconsistencies.is_empty() {
        let json_output = serde_json::to_string_pretty(&inconsistencies)?;
        let filename = "./tests/build/wrapped_interval_inconsistencies.json";
        let mut file = File::create(filename)?;
        file.write_all(json_output.as_bytes())?;
        println!("\nInconsistent results saved to: {}", filename);
    } else {
        println!("\nAll Rust implementations are consistent with the C++ baseline!");
    }

    Ok(())
}
