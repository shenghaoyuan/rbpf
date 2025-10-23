use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn debug_at_functions() {
    // Case 413: a = [792, 59], b = [552, 551]
    let a = WrappedRange::new_bounds(792, 59, 64);
    let b = WrappedRange::new_bounds(552, 551, 64);
    
    println!("=== Debug 'at' functions for case 413 ===");
    println!("a = [{}, {}] (wraps: {})", a.lb(), a.ub(), a.lb() > a.ub());
    println!("b = [{}, {}] (wraps: {})", b.lb(), b.ub(), b.lb() > b.ub());
    
    // 测试每个关键的 at 调用
    println!("\n=== Individual 'at' calls ===");
    
    // x.at(m_start): b.at(a.start)
    let b_at_792 = b.at(792);
    println!("b.at(792): Does [552,551] contain 792? {}", b_at_792);
    println!("  Check: 792 in [552, u64::MAX] ∪ [0, 551]?");
    println!("  792 >= 552? {} AND 792 <= u64::MAX? {} OR 792 >= 0? {} AND 792 <= 551? {}", 
             792 >= 552, 792 <= u64::MAX, 792 >= 0, 792 <= 551);
    
    // x.at(m_end): b.at(a.end)  
    let b_at_59 = b.at(59);
    println!("b.at(59): Does [552,551] contain 59? {}", b_at_59);
    println!("  Check: 59 in [552, u64::MAX] ∪ [0, 551]?");
    println!("  59 >= 552? {} AND 59 <= u64::MAX? {} OR 59 >= 0? {} AND 59 <= 551? {}", 
             59 >= 552, 59 <= u64::MAX, 59 >= 0, 59 <= 551);
    
    // at(x.m_start): a.at(b.start)
    let a_at_552 = a.at(552);
    println!("a.at(552): Does [792,59] contain 552? {}", a_at_552);
    println!("  Check: 552 in [792, u64::MAX] ∪ [0, 59]?");
    println!("  552 >= 792? {} AND 552 <= u64::MAX? {} OR 552 >= 0? {} AND 552 <= 59? {}", 
             552 >= 792, 552 <= u64::MAX, 552 >= 0, 552 <= 59);
    
    // at(x.m_end): a.at(b.end)
    let a_at_551 = a.at(551);
    println!("a.at(551): Does [792,59] contain 551? {}", a_at_551);
    println!("  Check: 551 in [792, u64::MAX] ∪ [0, 59]?");
    println!("  551 >= 792? {} AND 551 <= u64::MAX? {} OR 551 >= 0? {} AND 551 <= 59? {}", 
             551 >= 792, 551 <= u64::MAX, 551 >= 0, 551 <= 59);
    
    println!("\n=== Final condition for a <= b ===");
    let condition = b_at_792 && b_at_59 && (!a_at_552 || !a_at_551);
    println!("Condition: {} && {} && ({} || {}) = {}", 
             b_at_792, b_at_59, !a_at_552, !a_at_551, condition);
    
    println!("\n=== Expected vs Actual ===");
    println!("Expected (from C++ behavior): false");
    println!("Actual (Rust): {}", condition);
    
    if condition {
        println!("ERROR: Rust disagrees with C++!");
    } else {
        println!("SUCCESS: Rust matches C++!");
    }
}