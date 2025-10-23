use solana_sbpf::tnum::Tnum;

fn main() {
    println!("测试 fast_divide 函数");
    println!("==================");
    
    // 创建 Tnum(90, 0) 和 Tnum(3, 0)
    let dividend = Tnum::new(90, 0);  // 被除数：常数 90
    let divisor = Tnum::new(3, 0);    // 除数：常数 3
    
    println!("被除数: value={}, mask={} (常数 {})", dividend.value(), dividend.mask(), dividend.value());
    println!("除数:   value={}, mask={} (常数 {})", divisor.value(), divisor.mask(), divisor.value());
    
    // 计算 fast_divide 结果
    let result = dividend.fast_divide(divisor);
    
    println!("\nfast_divide 结果:");
    println!("  value={} (0b{:b})", result.value(), result.value());
    println!("  mask={} (0b{:b})", result.mask(), result.mask());
    
    // 计算真实除法结果用于对比
    let true_result = 90 / 3;
    println!("\n真实除法结果: {} ÷ {} = {}", 90, 3, true_result);
    
    // 检查真实结果是否在 fast_divide 结果范围内
    // 使用 Tnum 范围检查公式：(val XOR value) & !mask == 0
    let in_range = (true_result ^ result.value()) & !result.mask() == 0;
    println!("真实结果是否在 fast_divide 范围内: {}", in_range);
    
    if in_range {
        println!("✅ fast_divide 结果正确包含了真实结果");
    } else {
        println!("❌ fast_divide 结果未包含真实结果 - 存在错误!");
        
        // 详细分析
        println!("\n详细分析:");
        println!("  真实结果: {} (0b{:b})", true_result, true_result);
        println!("  fast_divide value: {} (0b{:b})", result.value(), result.value());
        println!("  fast_divide mask: {} (0b{:b})", result.mask(), result.mask());
        println!("  XOR 结果: {} (0b{:b})", true_result ^ result.value(), true_result ^ result.value());
        println!("  ~mask: {} (0b{:b})", !result.mask(), !result.mask());
        println!("  (XOR) & (~mask): {} (0b{:b})", (true_result ^ result.value()) & !result.mask(), (true_result ^ result.value()) & !result.mask());
    }
    
    // 也测试其他除法方法用于对比
    println!("\n=== 对比其他除法方法 ===");
    
    let sdiv_result = dividend.sdiv(divisor);
    println!("sdiv 结果: value={} (0b{:b}), mask={} (0b{:b})", 
             sdiv_result.value(), sdiv_result.value(), sdiv_result.mask(), sdiv_result.mask());
    
    let udiv_result = dividend.udiv(divisor);
    println!("udiv 结果: value={} (0b{:b}), mask={} (0b{:b})", 
             udiv_result.value(), udiv_result.value(), udiv_result.mask(), udiv_result.mask());
             
    // 检查这些方法的结果是否正确
    let sdiv_in_range = (true_result ^ sdiv_result.value()) & !sdiv_result.mask() == 0;
    let udiv_in_range = (true_result ^ udiv_result.value()) & !udiv_result.mask() == 0;
    
    println!("sdiv 结果正确: {}", sdiv_in_range);
    println!("udiv 结果正确: {}", udiv_in_range);
}
