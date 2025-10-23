use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    println!("=== Debug Case 15 ===");
    
    // Case 15 的具体输入
    let input_a = WrappedRange::new_bounds(448, 902, 64);
    let input_b = WrappedRange::new_bounds(90, 184, 64);
    
    println!("input_a: [{}, {}], is_bottom: {}", input_a.lb(), input_a.ub(), input_a.is_bottom());
    println!("input_b: [{}, {}], is_bottom: {}", input_b.lb(), input_b.ub(), input_b.is_bottom());
    
    // 执行and操作
    let result = input_a.and(&input_b);
    
    println!("result: [{}, {}], is_bottom: {}", result.lb(), result.ub(), result.is_bottom());
    println!("expected: [0, 0], is_bottom: true");
    
    // 验证at方法的计算
    println!("\n=== Manual verification ===");
    println!("input_b.at(448): {}", input_b.at(448));
    println!("input_a.at(90): {}", input_a.at(90));
    
    // 检查at方法的详细计算
    println!("\n=== AT method details ===");
    println!("For input_b.at(448):");
    println!("  value.wrapping_sub(lb): 448.wrapping_sub(90) = {}", 448u64.wrapping_sub(90));
    println!("  ub.wrapping_sub(lb): 184.wrapping_sub(90) = {}", 184u64.wrapping_sub(90));
    println!("  comparison: {} <= {} = {}", 448u64.wrapping_sub(90), 184u64.wrapping_sub(90), 448u64.wrapping_sub(90) <= 184u64.wrapping_sub(90));
    
    println!("For input_a.at(90):");
    println!("  value.wrapping_sub(lb): 90.wrapping_sub(448) = {}", 90u64.wrapping_sub(448));
    println!("  ub.wrapping_sub(lb): 902.wrapping_sub(448) = {}", 902u64.wrapping_sub(448));
    println!("  comparison: {} <= {} = {}", 90u64.wrapping_sub(448), 902u64.wrapping_sub(448), 90u64.wrapping_sub(448) <= 902u64.wrapping_sub(448));
    
    // 测试bottom方法
    println!("\n=== Bottom method test ===");
    let bottom = WrappedRange::bottom(64);
    println!("bottom: [{}, {}], is_bottom: {}", bottom.lb(), bottom.ub(), bottom.is_bottom());
}