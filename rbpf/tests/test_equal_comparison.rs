use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    println!("测试equal函数比较逻辑");
    println!("========================");
    
    // 测试用例382: C++结果 vs Rust结果
    let cpp_result = WrappedRange::new_bounds(0, 18446744073709551615, 64);
    let rust_result = WrappedRange::new_bounds(1, 0, 64);
    
    println!("C++结果: [0, 18446744073709551615]");
    println!("Rust结果: [1, 0]");
    println!();
    
    println!("C++结果状态:");
    println!("  is_top(): {}", cpp_result.is_top());
    println!("  is_bottom(): {}", cpp_result.is_bottom());
    println!();
    
    println!("Rust结果状态:");
    println!("  is_top(): {}", rust_result.is_top());
    println!("  is_bottom(): {}", rust_result.is_bottom());
    println!();
    
    // 使用equal函数比较
    let are_equal = cpp_result.equal(&rust_result);
    println!("equal()比较结果: {}", are_equal);
    
    // 测试其他情况
    println!("\n其他测试:");
    
    // 相同区间
    let same1 = WrappedRange::new_bounds(10, 20, 64);
    let same2 = WrappedRange::new_bounds(10, 20, 64);
    println!("相同区间 [10, 20] == [10, 20]: {}", same1.equal(&same2));
    
    // 不同区间
    let diff1 = WrappedRange::new_bounds(10, 20, 64);
    let diff2 = WrappedRange::new_bounds(15, 25, 64);
    println!("不同区间 [10, 20] == [15, 25]: {}", diff1.equal(&diff2));
    
    // 两个不同的top区间
    let top1 = WrappedRange::new_bounds(0, 18446744073709551615, 64);
    let top2 = WrappedRange::new_bounds(1, 0, 64);
    println!("不同top区间 [0, max] == [1, 0]: {}", top1.equal(&top2));
}
