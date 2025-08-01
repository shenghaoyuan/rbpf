//! This is a tnum implementation for Solana eBPF

// This is for bit-level abstraction
#[derive(Debug, Clone, Copy)]
/// tnum definition
pub struct Tnum {
    value: u64,
    mask: u64,
}

impl Tnum {
    /// 创建实例
    pub fn new(value: u64, mask: u64) -> Self {
        Self { value, mask }
    }
    
    /// 获取 value 字段
    pub fn value(&self) -> u64 {
        self.value
    }

    /// 获取 mask 字段
    pub fn mask(&self) -> u64 {
        self.mask
    }
}

/// 创建一个常数 tnum 实例
pub fn tnum_const(value: u64) -> Tnum {
    Tnum::new(value, 0)
}

/// from integer interval to tnum
pub fn tnum_range(min: u64, max: u64) -> Tnum {
    let chi = min ^ max;
    //最高未知位
    let bits = (64 - chi.leading_zeros()) as u64;
    //超出范围则完全未知
    if bits > 63 {
        return Tnum::new(0, u64::MAX);
    }

    //范围内的未知位
    let delta = (1u64 << bits) - 1;
    Tnum::new(min & !delta, delta)
}

/// tnum 的左移操作
pub fn tnum_lshift(a: Tnum, shift: u8) -> Tnum {
    Tnum::new(a.value.wrapping_shl(shift as u32), a.mask.wrapping_shl(shift as u32))
}

/// tnum 的右移操作
pub fn tnum_rshift(a: Tnum, shift: u8) -> Tnum {
    Tnum::new(a.value.wrapping_shr(shift as u32), a.mask.wrapping_shr(shift as u32))
}

/// tnum 算数右移的操作
pub fn tnum_arshift(a: Tnum, min_shift: u8, insn_bitness: u8) -> Tnum {
    match insn_bitness {
        32 => {
            //32位模式
            let value = ((a.value as i32) >> min_shift) as u32;
            let mask = ((a.mask as i32) >> min_shift) as u32;
            Tnum::new(value as u64, mask as u64)
        }
        _ => {
            //64位模式
            let value = ((a.value as i64) >> min_shift) as u64;
            let mask = ((a.mask as i64) >> min_shift) as u64;
            Tnum::new(value, mask)
        }
    }
}

/// tnum 的加法操作
pub fn tnum_add(a: Tnum, b: Tnum) -> Tnum {
    // 计算掩码之和 - 表示两个不确定数的掩码组合
    let sm = a.mask.wrapping_add(b.mask);

    // 计算确定值之和
    let sv = a.value.wrapping_add(b.value);

    // sigma = (a.mask + b.mask) + (a.value + b.value)
    // 用于检测进位传播情况
    let sigma = sm.wrapping_add(sv);

    // chi = 进位传播位图
    // 通过异或操作找出哪些位发生了进位
    let chi = sigma ^ sv;

    // mu = 最终的不确定位掩码
    // 包括:
    // 1. 进位产生的不确定性 (chi)
    // 2. 原始输入的不确定位 (a.mask | b.mask)
    let mu = chi | a.mask | b.mask;

    // 返回结果:
    // value: 确定值之和，但排除所有不确定位 (~mu)
    // mask: 所有不确定位的掩码
    Tnum::new(sv & !mu, mu)
}

/// tnum 的减法操作
pub fn tnum_sub(a: Tnum, b: Tnum) -> Tnum {
    let dv = a.value.wrapping_sub(b.value);
    let alpha = dv.wrapping_add(a.mask);
    let beta = dv.wrapping_sub(b.mask);
    let chi = alpha ^ beta;
    let mu = chi | a.mask | b.mask;
    Tnum::new(dv & !mu, mu)
}

/// tnum 的按位与操作
pub fn tnum_and(a: Tnum, b: Tnum) -> Tnum {
    let alpha = a.value | a.mask;
    let beta = b.value | b.mask;
    let v = a.value & b.value;

    Tnum::new(v, alpha & beta & !v)
}

/// tnum 的按位或操作
pub fn tnum_or(a: Tnum, b: Tnum) -> Tnum {
    let v = a.value | b.value;
    let mu = a.mask | b.mask;

    Tnum::new(v, mu & !v)
}

/// tnum 的按位异或操作
pub fn tnum_xor(a: Tnum, b: Tnum) -> Tnum {
    let v = a.value ^ b.value;
    let mu = a.mask | b.mask;

    Tnum::new(v & !mu, mu)
}

/// tnum 的乘法操作
pub fn tnum_mul(mut a: Tnum, mut b: Tnum) -> Tnum {
    let acc_v = a.value.wrapping_mul(b.value);
    let mut acc_m: Tnum = Tnum::new(0, 0);
    while (a.value != 0) || (a.mask != 0) {
        if (a.value & 1) != 0 {
            acc_m = tnum_add(acc_m, Tnum::new(0, b.mask));
        } else if (a.mask & 1) != 0 {
            acc_m = tnum_add(acc_m, Tnum::new(0, b.value | b.mask));
        }
        a = tnum_rshift(a, 1);
        b = tnum_lshift(b, 1);
    }
    tnum_add(Tnum::new(acc_v, 0), acc_m)
}

/// A constant-value optimization for tnum_mul
pub fn tnum_mul_opt(a: Tnum, b: Tnum) -> Tnum {
    // 如果一个是常数
    if a.mask == 0 && a.value.count_ones() == 1 { // a.value = 2 ^ x
        tnum_lshift(b, a.value.trailing_zeros() as u8)
    } else if b.mask == 0  && b.value.count_ones() == 1 { // a.value = 2 ^ x
        tnum_lshift(a, b.value.trailing_zeros() as u8)
    } else 
        if (a.value | a.mask).count_ones() <= (b.value | b.mask).count_ones() {
        tnum_mul(a, b)
    } else {
        tnum_mul(b, a)
    }
}

#[test]
fn test_tnum_mul () -> (){
    let a = Tnum::new(0b100, 0b011);
    let b = Tnum::new(0b111, 0b000);
    println!("{:?}", tnum_mul(a, b));
    println!("{:?}", tnum_mul_opt(a, b));
}


///computes the join of the tnum domain.
pub fn tnum_join (a: Tnum, b: Tnum) -> Tnum {
    let v = a.value ^ b.value;
    let m = (a.mask | b.mask) | v;
        Tnum::new((a.value | b.value) & (!m), m)
}

/// [split_at_mu] splits a tnum at the first unknow.
fn split_at_mu (x:Tnum) -> (Tnum, u32 , Tnum) {
    let i = x.mask.leading_ones();
    let x1 = Tnum::new(x.value >> (i+1), x.mask >> (i+1));
    let x2 = Tnum::new(x.value & ((1 << i) - 1), x.mask & ((1 << i) - 1));
        (x1,i,x2)
}

/// [tnum_mul_const] multiplies a constant [c] by the tnum [x]
/// which has [j] unknown bits and [n] is the fuel (Z.of_nat n = j).
fn tnum_mul_const (c:u64, x:Tnum, n:u64) -> Tnum {
    if n == 0 {
        Tnum::new(c.wrapping_mul(x.value), 0)
    } else {
        let (y1,i1,y2) = split_at_mu(x);
        let p = tnum_mul_const(c,y1,n-1);
        let mc = Tnum::new(c.wrapping_mul(y2.mask),0);
        let mu0 = tnum_add(tnum_lshift(p, (i1+1) as u8), mc);
        let mu1 = tnum_add(mu0, Tnum::new(c.wrapping_shl(i1),0));
           tnum_join(mu0, mu1)
    }

}

/// [xtnum_mul x i y j] computes the multiplication of
/// [x]  which has [i] unknown bits by
/// [y]  which has [j] unknown bits such (i <= j)
fn xtnum_mul (x:Tnum, i: u64, y:Tnum, j: u64) -> Tnum {
    if i == 0 && j == 0 {
        Tnum::new(x.value * y.value, 0)
    } else {
        let (y1,i1,y2) = split_at_mu(y); // y = y1.mu.y2
        let p = if i == j {
            xtnum_mul(y1, j-1, x, i)
        } else {
            xtnum_mul(x, i, y1, j-1)
        };
        let mc = tnum_mul_const(y2.value, x, i);
        let mu0 = tnum_add(tnum_lshift(p, (i1+1) as u8), mc);
        let mu1 = tnum_add(mu0, tnum_lshift(x, i1 as u8));
            tnum_join(mu0, mu1)
    }
}

/// the top of the xtnum_mul
pub fn xtnum_mul_top (x:Tnum, y:Tnum) -> Tnum {
    let i = 64 - x.mask.leading_zeros() as u64;
    let j = 64 - y.mask.leading_zeros() as u64;
        if i <= j {
            xtnum_mul(x, i, y, j)
        } else {
            xtnum_mul(y, j, x, i)
        }
}

/// clear bit of n-th
fn clear_bit(num: u64, pos: u8) -> u64 {
    num & !(1 << pos)
}

/// clear bit of a tnum
fn tnum_clearbit(x: Tnum, pos: u8) -> Tnum {
    Tnum::new(clear_bit(x.value, pos), clear_bit(x.mask, pos))
}

/// bit size of a tnum
fn tnum_size (x: Tnum) -> u8 {
    let a = 64 - x.value.leading_zeros();
    let b = 64 - x.mask.leading_zeros();
    if a < b {
        b as u8
    } else {
        a as u8
    }
}

/// max 64 of a tnum
fn tnum_max (a: Tnum) -> u64 {
    a.value | a.mask
}

/// check if the pos-th of num is 0 or 1
fn testbit(num: u64, pos: u8) -> bool {
    if pos >= 64 {
        false
    } else {
        (num & (1 << pos)) != 0
    }
}

/// [xtnum_mul_high x y n] multiplies x by y
/// where n is the number of bits that are set in either x or y.
/// We also have that x <= y and 0 <= x and 0 <= y
fn xtnum_mul_high (x: Tnum, y: Tnum, n: u8) -> Tnum {
    if x.mask == 0 && y.mask == 0 { //if both are constants, perform normal multiplication
        Tnum::new(x.value.wrapping_mul(y.value), 0)
    } else if n == 0 {
        //panic!("should not happen");
        Tnum::new(0, 0) //should not happen
    } else {
        let b = tnum_size(y);
        let ym = testbit(y.mask, b-1);
        let y_prime = tnum_clearbit(y, b-1); //clear the highest bit of y
        let p =
            if tnum_max(y_prime) <= tnum_max(x) {
                xtnum_mul_high(y_prime, x, n-1)
            } else {
                xtnum_mul_high(x, y_prime, n-1)
            };
            if ym {
                tnum_join(tnum_add(p,tnum_lshift(x, b-1)), p)
            } else {
                tnum_add(p, tnum_lshift(x, b-1))
            }
    }
}

/// the top level of xtnum_mul_high
pub fn xtnum_mul_high_top (x: Tnum, y: Tnum) -> Tnum {
    xtnum_mul_high(x, y,((x.value | x.mask).count_ones() + (y.value | y.mask).count_ones()) as u8)
}

#[test]
fn test_xtnum_mul () -> (){
    let a = Tnum::new(15, 0); // 2^4 - 1
    let b = Tnum::new(0, 31); // 2^5 - 1
    println!("{:?}", tnum_mul(a, b)); // Output: Tnum { value: 0, mask: 511 } 2^(4+5) -1
    println!("{:?}", xtnum_mul_top(a, b)); // Output: Tnum { value: 0, mask: 4095 }
    println!("{:?}", xtnum_mul_high_top(a, b)); // Tnum { value: 0, mask: 511 }
}


/// aux function for tnum_mul_rec
fn tnum_decompose (a: Tnum) -> (Tnum, Tnum) {
    (
        Tnum::new(a.value >> 1, a.mask >> 1),
        Tnum::new(a.value & 1, a.mask & 1)
    )
}

/// A new tnum_mul proposed by frederic
pub fn tnum_mul_rec(a: Tnum, b: Tnum) -> Tnum {
    if a.mask == 0 && b.mask == 0 {  // both are known
        Tnum::new(a.value * b.value, 0)
    } else if a.mask == u64::MAX && b.mask == u64::MAX { //both are unknown
        Tnum::new(0,u64::MAX)
    } else if (a.value == 0 && a.mask == 0) || (b.value == 0 && b.mask == 0) { // mult by 0
        Tnum::new(0, 0)
    } else if a.value == 1 && a.mask == 0 { // mult by 1
        b
    } else if b.value == 1 && b.mask == 0 { // mult by 1
        a
    } else {
        let (a_up,a_low) = tnum_decompose(a);
        let (b_up,b_low) = tnum_decompose(b);
        tnum_mul_rec(a_up, b_up)
        //tnum_mul_rec(a_up, b_up) + tnum_mul_rec(a_up, b_low) + tnum_mul_rec(a_low, b_up) + tnum_mul_rec(a_low, b_low)
        // TODO: this one is wrong, replace this line with the following impl
        /* decompose the mask of am && bm
        so that the last bits either 0s or 1s
        In assembly, finding the rightmost 1 or 0 of a number is fast

        let (a_up,a_low) = decompose a in
        let (b_up,b_low) = decompose b in
        // a_low and b_low are either 1s or 0s
        (mul a_up b_up) + (mul a_up b_low) +
        (mul a_low b_up) + (mul a_low b_low)
        */
    }

}

/// tnum 的交集计算
pub fn tnum_intersect(a: Tnum, b: Tnum) -> Tnum {
    let v = a.value | b.value;
    let mu = a.mask & b.mask;
    Tnum::new(v & !mu, mu)
}

/// tnum 用与截断到指定字节大小
pub fn tnum_cast(mut a: Tnum, size: u8) -> Tnum {
    //处理溢出
    a.value &= (1u64 << (size * 8)) - 1;
    a.mask &= (1u64 << (size * 8)) - 1;
    a
}

pub fn tnum_is_aligned(a: Tnum, size: u64) -> bool {
    if size == 0 {
        return true;
    } else {
        return ((a.value | a.mask) & (size - 1)) == 0;
    }
}


/// check if [b] is a subset of [a], that is
/// 1) for unknown bits: all bit-set in [b.mask] must exist in [a.mask]
/// 2) for known bits: all bit-set in [b.value] must exist in [a.value] or [a.mask]
pub fn tnum_in(a: Tnum, b: Tnum) -> bool {
    if (b.mask & !a.mask) != 0 {
        // if we find one bit-set in [b.mask] but not in [a.mask], return false
        return false;
    } else {
        // [(b.value & !a.mask)] removes all possible bit-set in [a.mask] from [b.value]
        // the rest part should be equal to [a.value]
        return a.value == (b.value & !a.mask);
    }
}

// pub fn xtnum_in(a: Tnum, b: Tnum) -> bool {
//     if (b.mask & !a.mask) != 0 {
//         return false;
//     } else {
//         return a.value == b.value;
//     }
// }

#[test]
fn test_tnum_in () -> (){
    let a = Tnum::new(1, 0);
    let b = Tnum::new(0, 1);
    println!("{:?}", tnum_in(b, a)); // true
    //println!("{:?}", xtnum_in(b, a)); // false
}

/// tnum转换为字符串
pub fn tnum_sbin(size: usize, mut a: Tnum) -> String {
    let mut result = vec![0u8; size];

    // 从高位到低位处理每一位
    for n in (1..=64).rev() {
        if n < size {
            result[n - 1] = match (a.mask & 1, a.value & 1) {
                (1, _) => b'x', // 不确定位
                (0, 1) => b'1', // 确定位 1
                (0, 0) => b'0', // 确定位 0
                _ => unreachable!(),
            };
        }
        // 右移处理下一位
        a.mask >>= 1;
        a.value >>= 1;
    }

    // 设置字符串结束位置
    let end = std::cmp::min(size - 1, 64);
    result[end] = 0;

    // 转换为字符串
    String::from_utf8(result[..end].to_vec()).unwrap_or_else(|_| String::new())
}

pub fn tnum_subreg(a: Tnum) -> Tnum {
    tnum_cast(a, 4)
}

pub fn tnum_clear_subreg(a: Tnum) -> Tnum {
    tnum_lshift(tnum_rshift(a, 32), 32)
}

pub fn tnum_with_subreg(reg: Tnum, subreg: Tnum) -> Tnum {
    tnum_or(tnum_clear_subreg(reg), tnum_subreg(subreg))
}

pub fn tnum_const_subreg(a: Tnum, value: u32) -> Tnum {
    tnum_with_subreg(a, tnum_const(value as u64))
}
