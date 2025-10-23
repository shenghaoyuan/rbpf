use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    println!("=== 调试keep_lower函数 ===");
    
    let case = WrappedRange::new_bounds(830, 1023, 64);
    println!("输入: [{}, {}] ({}位)", case.lb(), case.ub(), case.width());
    
    // 手动计算keep_lower
    let bits_to_keep = 63;
    let shift_amount = bits_to_keep + 1;  // 64
    println!("bits_to_keep: {}", bits_to_keep);
    println!("shift_amount: {}", shift_amount);
    
    if shift_amount >= 64 {
        println!("进入特殊处理分支");
        let modulus = 1u64 << bits_to_keep;  // 2^63
        println!("modulus = 2^63 = {}", modulus);
        
        let masked = case.lb() & 0xFFFFFFFFFFFFFFFF;
        println!("masked = {} & 0xFFFFFFFFFFFFFFFF = {}", case.lb(), masked);
        
        let result = masked % modulus;
        println!("result = {} % {} = {}", masked, modulus, result);
    } else {
        println!("进入正常分支");
        let mask = (1u64 << shift_amount) - 1;
        let masked = case.lb() & mask;
        let modulus = 1u64 << bits_to_keep;
        let result = masked % modulus;
        println!("mask = {}", mask);
        println!("masked = {}", masked);
        println!("modulus = {}", modulus);
        println!("result = {}", result);
    }
    
    // 测试实际的keep_lower函数
    println!("\n测试实际的keep_lower函数:");
    let result = case.trunc(63);
    println!("trunc结果: [{}, {}] ({}位)", result.lb(), result.ub(), result.width());
}
