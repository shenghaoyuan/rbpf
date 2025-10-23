// 测试新的CLAM兼容实现
fn get_signed_representation(value: u64, width: u32) -> i64 {
    let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
    let masked_value = value & mask;
    
    if masked_value & (1u64 << (width - 1)) != 0 {
        let sign_extended = masked_value | (!mask);
        sign_extended as i64
    } else {
        masked_value as i64
    }
}

fn wrapping_div_signed_clam_style(dividend: u64, divisor: u64, width: u32) -> u64 {
    let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
    let self_masked = dividend & mask;
    let rhs_masked = divisor & mask;
    
    if rhs_masked == 0 {
        panic!("wrapint: signed division by zero");
    }
    
    let dividend_signed = get_signed_representation(self_masked, width);
    let divisor_signed = get_signed_representation(rhs_masked, width);
    
    let result = dividend_signed / divisor_signed;
    (result as u64) & mask
}

fn main() {
    println!("Testing CLAM-compatible signed division implementation:");
    println!("Dividend\tDivisor\t\tResult\t\tExpected");
    
    let test_cases = vec![
        (10u64, 3u64, 64u32),
        (10u64, -3i64 as u64, 64u32),
        (-10i64 as u64, 3u64, 64u32),
        (-10i64 as u64, -3i64 as u64, 64u32),
        (7u64, 3u64, 64u32),
        (-7i64 as u64, 3u64, 64u32),
        (7u64, -3i64 as u64, 64u32),
        (-7i64 as u64, -3i64 as u64, 64u32),
        // 测试不同位宽
        (15u64, 4u64, 8u32),  // 8位：15 / 4 = 3
        (240u64, 4u64, 8u32), // 8位：-16 / 4 = -4 (240 = -16 in 8-bit)
    ];
    
    for (dividend, divisor, width) in test_cases {
        let result = wrapping_div_signed_clam_style(dividend, divisor, width);
        
        let dividend_signed = get_signed_representation(dividend, width);
        let divisor_signed = get_signed_representation(divisor, width);
        let expected = dividend_signed / divisor_signed;
        
        let result_signed = get_signed_representation(result, width);
        println!("{}\t\t{}\t\t{}\t\t{} ({})", 
                 dividend_signed, divisor_signed, 
                 result_signed, expected,
                 if result_signed == expected { "✓" } else { "✗" });
    }
    
    // 测试除零情况（应该panic）
    println!("\nTesting division by zero (should panic):");
    std::panic::catch_unwind(|| {
        wrapping_div_signed_clam_style(10, 0, 64);
    }).unwrap_err();
    println!("✓ Division by zero correctly panicked");
}
