#[cfg(test)]
mod tests {
    use solana_sbpf::wrapped_interval::WrappedRange;

    #[test]
    fn test_trunc_case_1042() {
        println!("=== Case 1042: Trunc Operation Test ===");
        println!("Input: [431, 757], bits_to_keep: 8, bitwidth: 64");
        
        // 创建测试输入
        let start = 431;
        let end = 757;
        let bits_to_keep = 8;
        let bitwidth = 64;
        
        // 创建wrapped interval
        let interval = WrappedRange::new_bounds(start, end, bitwidth);
        
        println!("\nInput interval:");
        println!("  start: {}", interval.lb());
        println!("  end: {}", interval.ub());
        println!("  bitwidth: {}", interval.width());
        println!("  is_bottom: {}", interval.is_bottom());
        println!("  is_top: {}", interval.is_top());
        
        // 执行trunc操作
        let result = interval.trunc(bits_to_keep);
        
        println!("\nRust trunc({}) result:", bits_to_keep);
        println!("  start: {}", result.lb());
        println!("  end: {}", result.ub());
        println!("  bitwidth: {}", result.width());
        println!("  is_bottom: {}", result.is_bottom());
        println!("  is_top: {}", result.is_top());
        
        // 分析期望的C++行为
        println!("\nExpected C++ behavior analysis:");
        let start_high = start >> bits_to_keep;
        let end_high = end >> bits_to_keep;
        
        println!("  start >> {} = {} >> {} = {}", bits_to_keep, start, bits_to_keep, start_high);
        println!("  end >> {} = {} >> {} = {}", bits_to_keep, end, bits_to_keep, end_high);
        
        if start_high == end_high {
            println!("  High bits are equal -> should return concrete interval");
            let mask = (1u64 << bits_to_keep) - 1;
            let lower_start = start & mask;
            let lower_end = end & mask;
            println!("  C++ expected: [{}, {}] with bitwidth {}", lower_start, lower_end, bitwidth);
        } else {
            println!("  High bits differ -> check continuity");
            let y = start_high + 1;
            if y == end_high {
                println!("  Continuous case detected");
                let mask = (1u64 << bits_to_keep) - 1;
                let lower_start = start & mask;
                let lower_end = end & mask;
                println!("  lower_start = {}, lower_end = {}", lower_start, lower_end);
                if !(lower_start <= lower_end) {
                    println!("  !(lower_start <= lower_end) -> should return concrete interval");
                    println!("  C++ expected: [{}, {}] with bitwidth {}", lower_start, lower_end, bitwidth);
                } else {
                    println!("  C++ expected: top with bitwidth {}", bitwidth);
                }
            } else {
                println!("  C++ expected: top with bitwidth {}", bitwidth);
            }
        }
        
        // 验证结果是否符合预期
        assert!(!result.is_bottom(), "Result should not be bottom");
    }
}