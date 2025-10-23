use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn test_case413_or_without_leq() {
    // Case 413: a = [792, 59], b = [552, 551]
    let a = WrappedRange::new_bounds(792, 59, 64);
    let b = WrappedRange::new_bounds(552, 551, 64);
    
    println!("=== Testing 'or' without less_or_equal check ===");
    println!("a = [{}, {}]", a.lb(), a.ub());
    println!("b = [{}, {}]", b.lb(), b.ub());
    
    // 手动实现 or 的复杂分支（跳过 less_or_equal 检查）
    let x_at_m_start = b.at(a.lb());
    let x_at_m_end = b.at(a.ub());
    let at_x_m_start = a.at(b.lb());
    let at_x_m_end = a.at(b.ub());
    
    println!("x.at(m_start): {} && x.at(m_end): {}", x_at_m_start, x_at_m_end);
    println!("at(x.m_start): {} && at(x.m_end): {}", at_x_m_start, at_x_m_end);
    
    // 第一个条件：all contain (should return top)
    let all_contain = x_at_m_start && x_at_m_end && at_x_m_start && at_x_m_end;
    println!("All contain condition: {}", all_contain);
    
    if all_contain {
        println!("Should return top");
    } else {
        // 第二个条件
        let cond2 = x_at_m_end && at_x_m_start;
        println!("Condition 2 (x.at(m_end) && at(x.m_start)): {}", cond2);
        
        // 第三个条件  
        let cond3 = at_x_m_end && x_at_m_start;
        println!("Condition 3 (at(x.m_end) && x.at(m_start)): {}", cond3);
        
        if cond2 {
            println!("Should return [m_start, x.m_end] = [{}, {}]", a.lb(), b.ub());
        } else if cond3 {
            println!("Should return [x.m_start, m_end] = [{}, {}]", b.lb(), a.ub());
        } else {
            // span 计算
            let span_a = b.lb().wrapping_sub(a.ub());
            let span_b = a.lb().wrapping_sub(b.ub());
            println!("span_a = x.m_start - m_end = {} - {} = {}", b.lb(), a.ub(), span_a);
            println!("span_b = m_start - x.m_end = {} - {} = {}", a.lb(), b.ub(), span_b);
            
            if span_a < span_b || (span_a == span_b && a.lb() <= b.lb()) {
                println!("Should return [m_start, x.m_end] = [{}, {}]", a.lb(), b.ub());
            } else {
                println!("Should return [x.m_start, m_end] = [{}, {}]", b.lb(), a.ub());
            }
        }
    }
    
    println!("\n=== Expected result ===");
    println!("C++ expects: [552, 59]");
    println!("This matches: Should return [x.m_start, m_end] from span comparison");
}