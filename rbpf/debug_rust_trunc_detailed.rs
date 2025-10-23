use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    println!("=== Rust Trunc Implementation Debug ===");
    
    // Case 1042 参数
    let start = 431u64;
    let end = 757u64;
    let bits_to_keep = 8u32;
    let width = 64u32;
    
    let interval = WrappedRange::new_bounds(start, end, width);
    
    println!("输入区间:");
    println!("  start: {}, end: {}, width: {}", start, end, width);
    println!("  start 二进制: {:064b}", start);
    println!("  end   二进制: {:064b}", end);
    println!("  bits_to_keep: {}", bits_to_keep);
    println!();
    
    // 检查 is_bottom 和 is_top
    println!("检查前置条件:");
    println!("  is_bottom: {}", interval.is_bottom());
    println!("  is_top: {}", interval.is_top());
    println!();
    
    // 计算高位
    let start_high = interval.lb().ashr(bits_to_keep as u64, width);
    let end_high = interval.ub().ashr(bits_to_keep as u64, width);
    
    println!("计算高位:");
    println!("  start_high = {}.ashr({}, {}) = {}", 
             interval.lb(), bits_to_keep, width, start_high);
    println!("  end_high = {}.ashr({}, {}) = {}", 
             interval.ub(), bits_to_keep, width, end_high);
    println!("  start_high == end_high: {}", start_high == end_high);
    println!();
    
    if start_high == end_high {
        println!("分支: 高位相同");
        let lower_start = interval.lb().keep_lower(bits_to_keep, width);
        let lower_end = interval.ub().keep_lower(bits_to_keep, width);
        
        println!("  lower_start = {}.keep_lower({}, {}) = {}", 
                 interval.lb(), bits_to_keep, width, lower_start);
        println!("  lower_end = {}.keep_lower({}, {}) = {}", 
                 interval.ub(), bits_to_keep, width, lower_end);
        println!("  lower_start <= lower_end: {}", lower_start <= lower_end);
        
        if lower_start <= lower_end {
            println!("  => 返回区间 [{}, {}]", lower_start, lower_end);
        } else {
            println!("  => 返回 top");
        }
    } else {
        println!("分支: 高位不同");
        let y = start_high.wrapping_add(1);
        println!("  y = start_high.wrapping_add(1) = {} + 1 = {}", start_high, y);
        println!("  y == end_high: {}", y == end_high);
        
        if y == end_high {
            println!("  -> 刚好跨越一个边界");
            let lower_start = interval.lb().keep_lower(bits_to_keep, width);
            let lower_end = interval.ub().keep_lower(bits_to_keep, width);
            
            println!("    lower_start = {}.keep_lower({}, {}) = {}", 
                     interval.lb(), bits_to_keep, width, lower_start);
            println!("    lower_end = {}.keep_lower({}, {}) = {}", 
                     interval.ub(), bits_to_keep, width, lower_end);
            println!("    lower_start <= lower_end: {}", lower_start <= lower_end);
            println!("    !(lower_start <= lower_end): {}", !(lower_start <= lower_end));
            
            if !(lower_start <= lower_end) {
                println!("    => 返回区间 [{}, {}] (低位无序)", lower_start, lower_end);
            } else {
                println!("    => 返回 top (低位有序)");
            }
        } else {
            println!("  -> 跨越多个边界");
            println!("  => 返回 top");
        }
    }
    
    println!();
    println!("实际 Rust trunc 结果:");
    let result = interval.trunc(bits_to_keep);
    println!("  result.is_top(): {}", result.is_top());
    println!("  result.is_bottom(): {}", result.is_bottom());
    if !result.is_top() && !result.is_bottom() {
        println!("  result: [{}, {}], width: {}", result.lb(), result.ub(), result.width());
    }
}