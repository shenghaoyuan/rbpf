use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn debug_at_computation() {
    // Case 413: a = [792, 59], b = [552, 551]
    let a = WrappedRange::new_bounds(792, 59, 64);
    let b = WrappedRange::new_bounds(552, 551, 64);
    
    println!("=== Debug at computation ===");
    println!("a = [{}, {}]", a.lb(), a.ub());
    println!("b = [{}, {}]", b.lb(), b.ub());
    
    // 手动计算 a.at(552)
    println!("\n=== Computing a.at(552) ===");
    let value = 552u64;
    let a_start = a.lb();
    let a_end = a.ub();
    
    println!("value = {}", value);
    println!("a_start = {}", a_start);
    println!("a_end = {}", a_end);
    
    let left_side = value.wrapping_sub(a_start);
    let right_side = a_end.wrapping_sub(a_start);
    
    println!("left_side = value - a_start = {} - {} = {}", value, a_start, left_side);
    println!("right_side = a_end - a_start = {} - {} = {}", a_end, a_start, right_side);
    println!("condition: {} <= {} = {}", left_side, right_side, left_side <= right_side);
    
    let result = a.at(552);
    println!("a.at(552) = {}", result);
    
    // 手动计算 a.at(551) 
    println!("\n=== Computing a.at(551) ===");
    let value2 = 551u64;
    let left_side2 = value2.wrapping_sub(a_start);
    println!("left_side = value - a_start = {} - {} = {}", value2, a_start, left_side2);
    println!("condition: {} <= {} = {}", left_side2, right_side, left_side2 <= right_side);
    
    let result2 = a.at(551);
    println!("a.at(551) = {}", result2);
    
    // 现在检查 C++ 返回 top 的条件
    println!("\n=== Checking C++ top condition ===");
    let x_at_m_start = b.at(a.lb());
    let x_at_m_end = b.at(a.ub());
    let at_x_m_start = a.at(b.lb());
    let at_x_m_end = a.at(b.ub());
    
    println!("x.at(m_start): b.at({}) = {}", a.lb(), x_at_m_start);
    println!("x.at(m_end): b.at({}) = {}", a.ub(), x_at_m_end);
    println!("at(x.m_start): a.at({}) = {}", b.lb(), at_x_m_start);
    println!("at(x.m_end): a.at({}) = {}", b.ub(), at_x_m_end);
    
    let top_condition = x_at_m_start && x_at_m_end && at_x_m_start && at_x_m_end;
    println!("Top condition: {} && {} && {} && {} = {}", 
             x_at_m_start, x_at_m_end, at_x_m_start, at_x_m_end, top_condition);
    
    if top_condition {
        println!("C++ should return top!");
    } else {
        println!("C++ should NOT return top");
    }
}