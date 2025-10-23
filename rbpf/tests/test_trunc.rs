use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    println!("测试trunc函数的基本功能");
    
    // 测试1: 基本截断
    let interval = WrappedRange::new_bounds(0x123456789ABCDEF0, 0x123456789ABCDEF5, 64);
    println!("原始区间: [{}, {}] (64位)", interval.lb(), interval.ub());
    
    let truncated_32 = interval.trunc(32);
    println!("截断到32位: [{}, {}] ({}位)", 
             truncated_32.lb(), truncated_32.ub(), truncated_32.width());
    
    let truncated_16 = interval.trunc(16);
    println!("截断到16位: [{}, {}] ({}位)", 
             truncated_16.lb(), truncated_16.ub(), truncated_16.width());
    
    // 测试2: 边界情况
    let top_interval = WrappedRange::top(64);
    println!("\nTop区间截断测试:");
    println!("原始: is_top={}, is_bottom={}", top_interval.is_top(), top_interval.is_bottom());
    
    let top_truncated = top_interval.trunc(32);
    println!("截断后: is_top={}, is_bottom={}, width={}", 
             top_truncated.is_top(), top_truncated.is_bottom(), top_truncated.width());
    
    // 测试3: 小区间截断
    let small_interval = WrappedRange::new_bounds(0x100, 0x200, 64);
    println!("\n小区间截断测试:");
    println!("原始: [{}, {}] (64位)", small_interval.lb(), small_interval.ub());
    
    let small_truncated = small_interval.trunc(8);
    println!("截断到8位: [{}, {}] ({}位)", 
             small_truncated.lb(), small_truncated.ub(), small_truncated.width());
    
    // 测试4: 验证位掩码操作
    println!("\n位掩码验证:");
    let value = 0x123456789ABCDEF0u64;
    let mask_32 = (1u64 << 32) - 1;
    let mask_16 = (1u64 << 16) - 1;
    let mask_8 = (1u64 << 8) - 1;
    
    println!("原始值: 0x{:X}", value);
    println!("32位掩码: 0x{:X} -> 0x{:X}", mask_32, value & mask_32);
    println!("16位掩码: 0x{:X} -> 0x{:X}", mask_16, value & mask_16);
    println!("8位掩码: 0x{:X} -> 0x{:X}", mask_8, value & mask_8);
}