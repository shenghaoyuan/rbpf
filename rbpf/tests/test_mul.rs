use solana_sbpf::wrapped_interval::WrappedRange;

#[test]
fn test_mul_basic() {
    // 测试基本乘法
    let a = WrappedRange::new_bounds(2, 4, 32);
    let b = WrappedRange::new_bounds(3, 5, 32);
    
    let result = a.mul(&b);
    println!("Test: [2,4] * [3,5] = {:?}", result);
    
    // 基本断言：结果不应该是 bottom
    assert!(!result.is_bottom());
}

#[test]
fn test_mul_bottom() {
    // 测试 bottom 情况
    let a = WrappedRange::bottom(32);
    let b = WrappedRange::new_bounds(3, 5, 32);
    
    let result = a.mul(&b);
    assert!(result.is_bottom());
}

#[test]
fn test_mul_top() {
    // 测试 top 情况
    let a = WrappedRange::top(32);
    let b = WrappedRange::new_bounds(3, 5, 32);
    
    let result = a.mul(&b);
    assert!(result.is_top());
}

#[test]
fn test_mul_constants() {
    // 测试常量乘法
    let a = WrappedRange::new_constant(3, 32);
    let b = WrappedRange::new_constant(4, 32);
    
    let result = a.mul(&b);
    println!("Test: 3 * 4 = {:?}", result);
    
    // 验证结果不是 bottom 或 top
    assert!(!result.is_bottom());
    assert!(!result.is_top());
}

fn main() {
    test_mul_basic();
    test_mul_bottom();
    test_mul_top();
    test_mul_constants();
    println!("All tests passed!");
}