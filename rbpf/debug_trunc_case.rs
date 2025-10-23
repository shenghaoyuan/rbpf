use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    println!("=== 分析trunc操作的不一致问题 ===");
    
    // 测试用例1354: [830, 1023] (64位) -> 截断到63位
    println!("\n测试用例1354:");
    let case1 = WrappedRange::new_bounds(830, 1023, 64);
    println!("输入: [{}, {}] ({}位)", case1.lb(), case1.ub(), case1.width());
    
    let result1 = case1.trunc(63);
    println!("Rust结果: [{}, {}] ({}位)", result1.lb(), result1.ub(), result1.width());
    println!("Rust is_bottom: {}", result1.is_bottom());
    println!("Rust is_top: {}", result1.is_top());
    
    // 分析高位部分
    let start_high = case1.lb() >> 63;
    let end_high = case1.ub() >> 63;
    println!("高位部分: start_high={}, end_high={}", start_high, end_high);
    println!("高位是否相同: {}", start_high == end_high);
    
    // 分析低位部分
    let lower_start = case1.lb() & ((1u64 << 63) - 1);
    let lower_end = case1.ub() & ((1u64 << 63) - 1);
    println!("低位部分: lower_start={}, lower_end={}", lower_start, lower_end);
    println!("低位关系: lower_start <= lower_end = {}", lower_start <= lower_end);
    
    println!("\n测试用例1386:");
    let case2 = WrappedRange::new_bounds(93, 677, 64);
    println!("输入: [{}, {}] ({}位)", case2.lb(), case2.ub(), case2.width());
    
    let result2 = case2.trunc(63);
    println!("Rust结果: [{}, {}] ({}位)", result2.lb(), result2.ub(), result2.width());
    println!("Rust is_bottom: {}", result2.is_bottom());
    println!("Rust is_top: {}", result2.is_top());
    
    // 分析高位部分
    let start_high2 = case2.lb() >> 63;
    let end_high2 = case2.ub() >> 63;
    println!("高位部分: start_high={}, end_high={}", start_high2, end_high2);
    println!("高位是否相同: {}", start_high2 == end_high2);
    
    // 分析低位部分
    let lower_start2 = case2.lb() & ((1u64 << 63) - 1);
    let lower_end2 = case2.ub() & ((1u64 << 63) - 1);
    println!("低位部分: lower_start={}, lower_end={}", lower_start2, lower_end2);
    println!("低位关系: lower_start <= lower_end = {}", lower_start2 <= lower_end2);
    
    // 分析为什么C++返回bottom
    println!("\n=== 分析C++返回bottom的原因 ===");
    println!("C++使用ashr(算术右移)而不是>>(逻辑右移)");
    println!("对于63位截断，C++检查的是第63位(符号位)");
    println!("830 >> 63 = {}", 830u64 >> 63);
    println!("1023 >> 63 = {}", 1023u64 >> 63);
    
    // 模拟C++的ashr行为
    let start_ashr = (830i64 as u64) >> 63;
    let end_ashr = (1023i64 as u64) >> 63;
    println!("模拟ashr: start_ashr={}, end_ashr={}", start_ashr, end_ashr);
    println!("ashr是否相同: {}", start_ashr == end_ashr);
}
