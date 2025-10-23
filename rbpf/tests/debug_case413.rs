use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn debug_case413() {
    // 测试用例 413
    let a = WrappedRange::new_bounds(792, 59, 64);
    let b = WrappedRange::new_bounds(552, 551, 64);
    
    println!("=== Case 413 Analysis ===");
    println!("a = [792, 59] (wrapping)");
    println!("b = [552, 551] (wrapping)");
    
    // 检查 a 和 b 是否包含彼此的端点
    println!("\nChecking containment:");
    println!("a.at(b.start=552): {}", a.at(552));  // a 是否包含 552
    println!("a.at(b.end=551): {}", a.at(551));    // a 是否包含 551
    println!("b.at(a.start=792): {}", b.at(792));  // b 是否包含 792
    println!("b.at(a.end=59): {}", b.at(59));      // b 是否包含 59
    
    // 检查小于等于关系
    println!("\nChecking less_or_equal:");
    println!("a <= b: {}", a.less_or_equal(&b));
    println!("b <= a: {}", b.less_or_equal(&a));
    
    // 执行 OR 操作
    let result = a.or(&b);
    println!("\nResult:");
    println!("Rust or result: [{}, {}]", result.lb(), result.ub());
    
    // 预期的 C++ 结果是 [0, 18446744073709551615] (即 top)
    println!("Expected (C++): [0, 18446744073709551615]");
    
    // 分析为什么应该是 top
    println!("\n=== Analysis ===");
    
    // 检查区间的包含关系
    let all_conditions = b.at(792) && b.at(59) && a.at(552) && a.at(551);
    println!("All containment conditions (should trigger top): {}", all_conditions);
    
    if all_conditions {
        println!("Both intervals contain each other's endpoints -> should return top");
    } else {
        println!("Analyzing other conditions...");
        
        // 检查其他条件
        let cond1 = b.at(59) && a.at(552);  // x.at(m_end) && at(x.m_start)
        let cond2 = a.at(551) && b.at(792); // at(x.m_end) && x.at(m_start)
        
        println!("Condition 1 (b.at(a.end) && a.at(b.start)): {}", cond1);
        println!("Condition 2 (a.at(b.end) && b.at(a.start)): {}", cond2);
        
        if cond1 {
            println!("Should return [a.start, b.end] = [792, 551]");
        } else if cond2 {
            println!("Should return [b.start, a.end] = [552, 59]");
        } else {
            // 计算 span
            let span_a = b.lb().wrapping_sub(a.ub());  // x.start - self.end
            let span_b = a.lb().wrapping_sub(b.ub());  // self.start - x.end
            println!("span_a (b.start - a.end): {} - {} = {}", b.lb(), a.ub(), span_a);
            println!("span_b (a.start - b.end): {} - {} = {}", a.lb(), b.ub(), span_b);
            
            if span_a < span_b || (span_a == span_b && a.lb() <= b.lb()) {
                println!("Should return [a.start, b.end] = [792, 551]");
            } else {
                println!("Should return [b.start, a.end] = [552, 59]");
            }
        }
    }
}