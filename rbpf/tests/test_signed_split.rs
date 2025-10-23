use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    println!("=== 测试 signed_split 方法 ===");
    
    // 测试用例1: top 情况
    let top_interval = WrappedRange::top(64);
    let mut intervals = Vec::new();
    top_interval.signed_split(&mut intervals);
    
    println!("Top interval split:");
    for (i, interval) in intervals.iter().enumerate() {
        println!("  区间{}: [{}, {}], is_bottom: {}", 
                 i, interval.lb(), interval.ub(), interval.is_bottom());
    }
    
    // 测试用例2: 不跨越边界的区间 [100, 200]
    let normal_interval = WrappedRange::new_bounds(100, 200, 64);
    let mut intervals2 = Vec::new();
    normal_interval.signed_split(&mut intervals2);
    
    println!("\nNormal interval [100, 200] split:");
    for (i, interval) in intervals2.iter().enumerate() {
        println!("  区间{}: [{}, {}], is_bottom: {}", 
                 i, interval.lb(), interval.ub(), interval.is_bottom());
    }
    
    // 测试用例3: 跨越有符号边界的区间 [2^63-10, 2^63+10]
    let signed_max = WrappedRange::get_signed_max(64); // 2^63-1
    let signed_min = WrappedRange::get_signed_min(64); // 2^63
    let cross_boundary = WrappedRange::new_bounds(signed_max - 5, signed_min + 5, 64);
    let mut intervals3 = Vec::new();
    cross_boundary.signed_split(&mut intervals3);
    
    println!("\nCross boundary interval [{}, {}] split:", signed_max - 5, signed_min + 5);
    for (i, interval) in intervals3.iter().enumerate() {
        println!("  区间{}: [{}, {}], is_bottom: {}", 
                 i, interval.lb(), interval.ub(), interval.is_bottom());
    }
    
    // 测试 signed_limit 的构造
    let signed_limit = WrappedRange::new_bounds(
        WrappedRange::get_signed_max(64),
        WrappedRange::get_signed_min(64),
        64,
    );
    println!("\nSigned limit interval: [{}, {}]", signed_limit.lb(), signed_limit.ub());
    
    // 验证 signed_limit <= cross_boundary （注释掉因为方法是私有的）
    // let is_less_or_equal = signed_limit.less_or_equal(&cross_boundary);
    // println!("signed_limit <= cross_boundary: {}", is_less_or_equal);
    
    // 打印关键值
    println!("\n=== 关键值 ===");
    println!("Width 64:");
    println!("  get_unsigned_min(64) = {}", WrappedRange::get_unsigned_min(64));
    println!("  get_signed_max(64) = {}", WrappedRange::get_signed_max(64));
    println!("  get_signed_min(64) = {}", WrappedRange::get_signed_min(64));
    println!("  get_unsigned_max(64) = {}", WrappedRange::get_unsigned_max(64));
}