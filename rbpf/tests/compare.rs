// 文件名: src/compare.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use solana_sbpf::tnum::Tnum;

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
    input_a: TnumValue,
    input_b: TnumValue,
    baseline_output: TnumValue,
    rust_output: TnumValue,
    method: String,
}

#[derive(Clone, Debug)]
struct TnumValue {
    value: u64,
    mask: u64,
}

impl Serialize for TnumValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("TnumValue", 2)?;
        state.serialize_field("value", &format!("{:064b}", self.value))?;
        state.serialize_field("mask", &format!("{:064b}", self.mask))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for TnumValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            value: u64,
            mask: u64,
        }
        let helper = Helper::deserialize(deserializer)?;
        Ok(TnumValue {
            value: helper.value,
            mask: helper.mask,
        })
    }
}

impl std::fmt::Display for TnumValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(value: {:064b}, mask: {:064b})", self.value, self.mask)
    }
}

#[derive(Deserialize)]
struct TestCase {
    operation: String,
    input_a: TnumValue,
    input_b: TnumValue,
    results: Vec<MethodResult>,
}

#[derive(Deserialize)]
struct MethodResult {
    method: String,
    output: TnumValue,
    avg_time_ns: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <cpp_test_results.json>", args[0]);
        return Err("Insufficient arguments".into());
    }

    let input_file = &args[1];
    let json_data = fs::read_to_string(input_file)?;
    let test_cases: Vec<TestCase> = serde_json::from_str(&json_data)?;

    println!("Analyzing {} test cases...", test_cases.len());

    let mut stats: HashMap<String, MethodStats> = HashMap::new();
    let mut inconsistencies: Vec<Inconsistency> = Vec::new();

    for (i, test_case) in test_cases.iter().enumerate() {
        let baseline_method_name = format!("CPP_{}", test_case.operation);
        let baseline_result = test_case.results.iter().find(|r| r.method == baseline_method_name);

        if let Some(baseline) = baseline_result {
            // Update stats for the baseline method
            let s = stats.entry(baseline.method.clone()).or_insert_with(|| MethodStats::new(&baseline.method));
            s.total_count += 1;
            s.total_time += baseline.avg_time_ns;
            s.equal +=1;

            for result in &test_case.results {
                if result.method.starts_with("CPP_") {
                    continue;
                }
                
                let s = stats.entry(result.method.clone()).or_insert_with(|| MethodStats::new(&result.method));
                s.total_count += 1;
                s.total_time += result.avg_time_ns;

                let correct = result.output.value == baseline.output.value && result.output.mask == baseline.output.mask;

                if correct {
                    s.equal += 1;
                } else {
                    let baseline_tnum = Tnum::new(baseline.output.value, baseline.output.mask);
                    let result_tnum = Tnum::new(result.output.value, result.output.mask);
                    if baseline_tnum.contains(result_tnum) {
                        s.less_than += 1;
                    } else if result_tnum.contains(baseline_tnum) {
                        s.more_than += 1;
                    } else {
                        s.not_equal += 1;
                    }
                    inconsistencies.push(Inconsistency {
                        case_number: (i + 1) as u32,
                        operation: test_case.operation.clone(),
                        input_a: test_case.input_a.clone(),
                        input_b: test_case.input_b.clone(),
                        baseline_output: baseline.output.clone(),
                        rust_output: result.output.clone(),
                        method: result.method.clone(),
                    });
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
        let filename = "./tests/build/inconsistencies.json";
        let mut file = File::create(filename)?;
        file.write_all(json_output.as_bytes())?;
        println!("\nInconsistent results saved to: {}", filename);
    } else {
        println!("\nAll Rust implementations are consistent with the C++ baseline!");
    }

    Ok(())
}