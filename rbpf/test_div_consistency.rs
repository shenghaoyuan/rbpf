// 直接实现 wrapping_div_signed 进行测试
fn wrapping_div_signed(dividend: u64, divisor: u64, width: u32) -> u64 {
    if width == 0 || width > 64 {
        return 0;
    }
    
    let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
    let self_masked = dividend & mask;
    let rhs_masked = divisor & mask;
    
    if rhs_masked == 0 {
        return 0;
    }
    
    let self_signed = if self_masked & (1u64 << (width - 1)) != 0 {
        (self_masked as i64) | (!mask as i64)
    } else {
        self_masked as i64
    };
    
    let rhs_signed = if rhs_masked & (1u64 << (width - 1)) != 0 {
        (rhs_masked as i64) | (!mask as i64)
    } else {
        rhs_masked as i64
    };
    
    let result_signed = self_signed.wrapping_div(rhs_signed);
    (result_signed as u64) & mask
}

fn main() {
    // 测试一些基本的有符号除法案例
    let test_cases = vec![
        // (dividend, divisor, width, expected_result)
        (10u64, 3u64, 64u32),     // 正数 / 正数
        (10u64, -3i64 as u64, 64u32),  // 正数 / 负数  
        (-10i64 as u64, 3u64, 64u32),  // 负数 / 正数
        (-10i64 as u64, -3i64 as u64, 64u32), // 负数 / 负数
        (7u64, 3u64, 64u32),      // 有余数的情况
        (-7i64 as u64, 3u64, 64u32),
        (7u64, -3i64 as u64, 64u32),
        (-7i64 as u64, -3i64 as u64, 64u32),
    ];
    
    println!("Testing wrapping_div_signed consistency:");
    println!("Dividend\tDivisor\t\tResult\t\tExpected(i64)");
    
    for (dividend, divisor, width) in test_cases {
        let result = wrapping_div_signed(dividend, divisor, width);
        
        // 手动计算期望结果
        let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
        let dividend_masked = dividend & mask;
        let divisor_masked = divisor & mask;
        
        let dividend_signed = if dividend_masked & (1u64 << (width - 1)) != 0 {
            (dividend_masked as i64) | (!mask as i64)
        } else {
            dividend_masked as i64
        };
        
        let divisor_signed = if divisor_masked & (1u64 << (width - 1)) != 0 {
            (divisor_masked as i64) | (!mask as i64)
        } else {
            divisor_masked as i64
        };
        
        let expected_signed = dividend_signed / divisor_signed;
        let expected = (expected_signed as u64) & mask;
        
        println!("{}\t\t{}\t\t{}\t\t{} ({})", 
                 dividend_signed, divisor_signed, 
                 result as i64, expected_signed,
                 if result == expected { "✓" } else { "✗" });
    }
}
