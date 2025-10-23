use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn test_shl_const_debug() {
    // 测试用例：{start: 683, end: 35, bitwidth: 64}
    let a = WrappedRange::new_bounds(683, 35, 64);
    println!("输入区间: start={}, end={}, bitwidth={}, is_bottom={}", 
             a.lb(), a.ub(), a.width(), a.is_bottom());
    
    // 测试 shl_const(13)
    let result = a.shl_const(13);
    println!("shl_const(13) 结果: start={}, end={}, bitwidth={}, is_bottom={}", 
             result.lb(), result.ub(), result.width(), result.is_bottom());
    
    // 测试 trunc 操作
    let b = a.width();
    let k = 13u64;
    let truncated = a.trunc(b - k as u32);
    println!("trunc({}) 结果: start={}, end={}, bitwidth={}, is_bottom={}, is_top={}", 
             b - k as u32, truncated.lb(), truncated.ub(), truncated.width(), 
             truncated.is_bottom(), truncated.is_top());
}

#[test]
fn test_shl_const_multiple_cases() {
    // 测试多个不一致的案例
    let test_cases = vec![
        (683, 35, 13),
        (428, 352, 42),
        (842, 378, 34),
        (953, 201, 18),
    ];
    
    for (start, end, shift) in test_cases {
        println!("\n=== 测试案例: start={}, end={}, shift={} ===", start, end, shift);
        
        let a = WrappedRange::new_bounds(start, end, 64);
        println!("输入区间: start={}, end={}, bitwidth={}, is_bottom={}, is_top={}", 
                 a.lb(), a.ub(), a.width(), a.is_bottom(), a.is_top());
        
        // 测试 trunc 操作
        let b = a.width();
        let truncated = a.trunc(b - shift as u32);
        println!("trunc({}) 结果: start={}, end={}, bitwidth={}, is_bottom={}, is_top={}", 
                 b - shift as u32, truncated.lb(), truncated.ub(), truncated.width(), 
                 truncated.is_bottom(), truncated.is_top());
        
        // 测试 shl_const 操作
        let result = a.shl_const(shift);
        println!("shl_const({}) 结果: start={}, end={}, bitwidth={}, is_bottom={}, is_top={}", 
                 shift, result.lb(), result.ub(), result.width(), 
                 result.is_bottom(), result.is_top());
    }
}

#[test]
fn test_ashr_methods() {
    println!("\n=== 测试 ashr 方法 ===");
    
    // 测试正常区间
    let a = WrappedRange::new_bounds(100, 200, 64);
    println!("输入区间: start={}, end={}, bitwidth={}", a.lb(), a.ub(), a.width());
    
    // 测试 ashr_const
    let result1 = a.ashr_const(2);
    println!("ashr_const(2) 结果: start={}, end={}, bitwidth={}, is_bottom={}, is_top={}", 
             result1.lb(), result1.ub(), result1.width(), result1.is_bottom(), result1.is_top());
    
    // 测试环绕区间
    let b = WrappedRange::new_bounds(683, 35, 64);
    println!("\n环绕区间: start={}, end={}, bitwidth={}", b.lb(), b.ub(), b.width());
    
    let result2 = b.ashr_const(2);
    println!("ashr_const(2) 结果: start={}, end={}, bitwidth={}, is_bottom={}, is_top={}", 
             result2.lb(), result2.ub(), result2.width(), result2.is_bottom(), result2.is_top());
    
    // 测试区间位移
    let shift_interval = WrappedRange::new_bounds(2, 2, 64); // 单例区间
    let result3 = a.ashr(&shift_interval);
    println!("ashr(单例区间2) 结果: start={}, end={}, bitwidth={}, is_bottom={}, is_top={}", 
             result3.lb(), result3.ub(), result3.width(), result3.is_bottom(), result3.is_top());
}

#[test]
fn test_cross_signed_limit_concept() {
    println!("\n=== 测试 cross_signed_limit 概念 ===");
    
    // 64位有符号数的范围
    let signed_max_64 = 9223372036854775807u64; // 2^63 - 1
    let signed_min_64 = 9223372036854775808u64; // 2^63 (作为无符号数表示)
    
    println!("64位有符号数范围:");
    println!("  signed_max = {}", signed_max_64);
    println!("  signed_min = {} (作为无符号数)", signed_min_64);
    
    // 测试不同的区间类型
    let test_cases = vec![
        ("正常区间", 100, 200),
        ("环绕区间1", 683, 35),
        ("环绕区间2", 428, 352),
        ("跨越signed_max的区间", signed_max_64 - 100, signed_max_64 + 100),
        ("跨越signed_min的区间", signed_min_64 - 100, signed_min_64 + 100),
        ("完全在正数范围", 1000, 2000),
        ("完全在负数范围", signed_min_64 + 1000, signed_min_64 + 2000),
    ];
    
    for (name, start, end) in test_cases {
        let interval = WrappedRange::new_bounds(start, end, 64);
        println!("\n{}: [{}, {}]", name, start, end);
        println!("  is_wrapping: {}", start > end);
        println!("  cross_signed_limit: {}", interval.cross_signed_limit());
        
        // 显示signed_limit区间
        let signed_limit = interval.signed_limit(64);
        println!("  signed_limit: [{}, {}]", signed_limit.lb(), signed_limit.ub());
    }
}
