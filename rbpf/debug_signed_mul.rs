use solana_sbpf::wrapped_interval::WrappedRange;

fn main() {
    // 测试用例：[932, 559] * [957, 200]
    let a = WrappedRange::new_bounds(932, 559, 64);
    let b = WrappedRange::new_bounds(957, 200, 64);
    
    println!("Input A: {:?}", a);
    println!("Input B: {:?}", b);
    
    // 检查MSB状态
    println!("A.start MSB: {}", (a.get_start() >> 63) & 1 == 1);
    println!("A.end MSB: {}", (a.get_end() >> 63) & 1 == 1);
    println!("B.start MSB: {}", (b.get_start() >> 63) & 1 == 1);
    println!("B.end MSB: {}", (b.get_end() >> 63) & 1 == 1);
    
    // 检查是否为环绕区间
    println!("A is wrapped: {}", a.get_start() > a.get_end());
    println!("B is wrapped: {}", b.get_start() > b.get_end());
    
    let result = a.signed_mul(&b);
    println!("Signed_mul result: {:?}", result);
    
    // 手动计算期望的结果
    println!("\n手动计算:");
    println!("932 * 957 = {}", 932u64.wrapping_mul(957));
    println!("932 * 200 = {}", 932u64.wrapping_mul(200));
    println!("559 * 957 = {}", 559u64.wrapping_mul(957));
    println!("559 * 200 = {}", 559u64.wrapping_mul(200));
}