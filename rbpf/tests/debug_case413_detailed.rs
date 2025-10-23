use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn debug_case413_detailed() {
    // 测试用例 413
    let a = WrappedRange::new_bounds(792, 59, 64);
    let b = WrappedRange::new_bounds(552, 551, 64);
    
    println!("=== Case 413 Detailed Analysis ===");
    println!("a = [792, 59] (wrapping)");
    println!("b = [552, 551] (wrapping)");
    
    // 计算正确的包含关系
    // a 是 [792, 59]，即从 792 到 u64::MAX，然后从 0 到 59
    // b 是 [552, 551]，即从 552 到 u64::MAX，然后从 0 到 551
    
    println!("\n=== Detailed containment analysis ===");
    println!("a contains 552? {}", a.at(552));  // 552 在 [792, u64::MAX] 中吗？
    println!("a contains 551? {}", a.at(551));  // 551 在 [0, 59] 中吗？
    println!("b contains 792? {}", b.at(792));  // 792 在 [552, u64::MAX] 中吗？
    println!("b contains 59? {}", b.at(59));    // 59 在 [0, 551] 中吗？
    
    // 根据包含关系分析
    let x_at_m_start = b.at(a.lb());  // x.at(m_start)
    let x_at_m_end = b.at(a.ub());    // x.at(m_end)
    let at_x_m_start = a.at(b.lb());  // at(x.m_start)
    let at_x_m_end = a.at(b.ub());    // at(x.m_end)
    
    println!("\n=== C++ operator<= conditions ===");
    println!("x.at(m_start): b.at({}) = {}", a.lb(), x_at_m_start);
    println!("x.at(m_end): b.at({}) = {}", a.ub(), x_at_m_end);
    println!("at(x.m_start): a.at({}) = {}", b.lb(), at_x_m_start);
    println!("at(x.m_end): a.at({}) = {}", b.ub(), at_x_m_end);
    
    let not_at_x_start = !at_x_m_start;
    let not_at_x_end = !at_x_m_end;
    
    println!("!at(x.m_start): !a.at({}) = {}", b.lb(), not_at_x_start);
    println!("!at(x.m_end): !a.at({}) = {}", b.ub(), not_at_x_end);
    
    let condition_for_leq = x_at_m_start && x_at_m_end && (not_at_x_start || not_at_x_end);
    println!("a <= b condition: {} && {} && ({} || {}) = {}", 
             x_at_m_start, x_at_m_end, not_at_x_start, not_at_x_end, condition_for_leq);
    
    println!("\n=== C++ operator| conditions ===");
    // 检查 C++ 的 operator| 所有条件
    
    // 第一个条件：x.at(m_start) && x.at(m_end) && at(x.m_start) && at(x.m_end)
    let all_contain = x_at_m_start && x_at_m_end && at_x_m_start && at_x_m_end;
    println!("All contain condition (should return top): {}", all_contain);
    
    if all_contain {
        println!("C++ should return top!");
    } else {
        // 第二个条件：x.at(m_end) && at(x.m_start)
        let cond2 = x_at_m_end && at_x_m_start;
        println!("Condition 2 (x.at(m_end) && at(x.m_start)): {}", cond2);
        
        // 第三个条件：at(x.m_end) && x.at(m_start)
        let cond3 = at_x_m_end && x_at_m_start;
        println!("Condition 3 (at(x.m_end) && x.at(m_start)): {}", cond3);
        
        if cond2 {
            println!("C++ should return [m_start, x.m_end] = [{}, {}]", a.lb(), b.ub());
        } else if cond3 {
            println!("C++ should return [x.m_start, m_end] = [{}, {}]", b.lb(), a.ub());
        } else {
            // span 计算
            let span_a = b.lb().wrapping_sub(a.ub());
            let span_b = a.lb().wrapping_sub(b.ub());
            println!("span_a = x.m_start - m_end = {} - {} = {}", b.lb(), a.ub(), span_a);
            println!("span_b = m_start - x.m_end = {} - {} = {}", a.lb(), b.ub(), span_b);
            
            if span_a < span_b || (span_a == span_b && a.lb() <= b.lb()) {
                println!("C++ should return [m_start, x.m_end] = [{}, {}]", a.lb(), b.ub());
            } else {
                println!("C++ should return [x.m_start, m_end] = [{}, {}]", b.lb(), a.ub());
            }
        }
    }
    
    // 执行 Rust 的 or 操作
    let rust_result = a.or(&b);
    println!("\nRust result: [{}, {}]", rust_result.lb(), rust_result.ub());
}