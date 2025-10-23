use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn test_case413_leq() {
    // Case 413: a = [792, 59], b = [552, 551]
    let a = WrappedRange::new_bounds(792, 59, 64);
    let b = WrappedRange::new_bounds(552, 551, 64);
    
    println!("=== Testing less_or_equal for case 413 ===");
    println!("a = [{}, {}]", a.lb(), a.ub());
    println!("b = [{}, {}]", b.lb(), b.ub());
    
    // 检查 at 函数的返回值
    let x_at_m_start = b.at(a.lb());  
    let x_at_m_end = b.at(a.ub());    
    let at_x_m_start = a.at(b.lb());  
    let at_x_m_end = a.at(b.ub());    
    
    println!("b.at(a.start): b.at({}) = {}", a.lb(), x_at_m_start);
    println!("b.at(a.end): b.at({}) = {}", a.ub(), x_at_m_end);
    println!("a.at(b.start): a.at({}) = {}", b.lb(), at_x_m_start);
    println!("a.at(b.end): a.at({}) = {}", b.ub(), at_x_m_end);
    
    // 计算条件
    let condition = x_at_m_start && x_at_m_end && (!at_x_m_start || !at_x_m_end);
    println!("Condition: {} && {} && ({} || {}) = {}", 
             x_at_m_start, x_at_m_end, !at_x_m_start, !at_x_m_end, condition);
    
    // 调用 less_or_equal
    let result = a.less_or_equal(&b);
    println!("a.less_or_equal(b) = {}", result);
    
    // 根据 C++ 的行为，这应该返回 false
    if result {
        println!("WARNING: Rust returns true, but C++ likely returns false!");
        println!("This would cause 'or' to take the wrong branch.");
    }
}