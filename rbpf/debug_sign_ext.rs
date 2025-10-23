fn main() {
    let value = 240u64;  // 这在8位下应该是-16
    let width = 8u32;
    
    println!("Debug sign extension for value={}, width={}", value, width);
    
    let mask = (1u64 << width) - 1;  // 0xFF for 8-bit
    let masked_value = value & mask;  // 240 & 0xFF = 240 = 0xF0
    
    println!("mask = 0x{:X}", mask);
    println!("masked_value = 0x{:X} ({})", masked_value, masked_value);
    
    let sign_bit = 1u64 << (width - 1);  // 0x80 for 8-bit
    println!("sign_bit = 0x{:X}", sign_bit);
    
    let is_negative = (masked_value & sign_bit) != 0;
    println!("is_negative = {}", is_negative);
    
    if is_negative {
        let sign_extended = masked_value | (!mask);
        println!("!mask = 0x{:X}", !mask);
        println!("sign_extended = 0x{:X}", sign_extended);
        println!("as i64 = {}", sign_extended as i64);
        
        // 正确的符号扩展应该是：
        let correct = (masked_value as i8) as i64;
        println!("correct (via i8) = {}", correct);
    }
}


