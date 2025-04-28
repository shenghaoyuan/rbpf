#[derive(Debug, Clone, Copy)]
pub struct Tnum {
    value: u64,
    mask: u64,
}

impl Tnum {
    /// 创建实例
    pub fn new(value: u64, mask: u64) -> Self {
        Self { value, mask }
    }
}

/// 创建一个常数 tnum 实例
pub fn tnum_const(value: u64) -> Tnum {
    Tnum::new(value, 0)
}


pub fn tnum_range(min: u64, max: u64) -> Tnum {
    let chi = min^max;
    //最高未知位
    let bits = (64-chi.leading_zeros()) as u64;
    //超出范围则完全未知
    if bits > 63{
        return Tnum::new(0,u64::MAX)
    }

    //范围内的未知位
    let delta = (1u64 << bits) - 1;
    Tnum::new(min & !delta, delta)
}

/// tnum 的左移操作
pub fn tnum_lshift(a:Tnum, shift: u8) -> Tnum {
    Tnum::new(a.value << shift, a.mask << shift)
}

/// tnum 的右移操作
pub fn tnum_rshift(a:Tnum, shift: u8) -> Tnum {
    Tnum::new(a.value >> shift, a.mask >> shift)
}

/// tnum 算数右移的操作
pub fn tnum_arshift(a:Tnum,min_shift:u8,insn_bitness:u8)->Tnum{
    match insn_bitness{
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
pub fn tnum_add(a:Tnum, b:Tnum) -> Tnum {
    // 计算掩码之和 - 表示两个不确定数的掩码组合
    let sm = a.mask + b.mask;
    
    // 计算确定值之和
    let sv = a.value + b.value;
    
    // sigma = (a.mask + b.mask) + (a.value + b.value)
    // 用于检测进位传播情况
    let sigma = sm + sv;
    
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
pub fn tnum_sub(a:Tnum, b:Tnum) -> Tnum {
    let dv = a.value-b.value;
    let alpha = dv+a.mask;
    let beta = dv-b.mask;
    let chi = alpha^beta;
    let mu = chi|a.mask|b.mask;
    Tnum::new(dv & !mu, mu)
}


/// tnum 的按位与操作
pub fn tnum_and(a:Tnum, b:Tnum) -> Tnum {
    let alpha = a.value|a.mask;
    let beta = b.value|b.mask;
    let v = a.value&b.value;

    Tnum::new(v, alpha&beta&!v)
}


/// tnum 的按位或操作
pub fn tnum_or(a:Tnum, b:Tnum) -> Tnum {
    let v = a.value|b.value;
    let mu = a.mask|b.mask;

    Tnum::new(v, mu&!v)
}

/// tnum 的按位异或操作
pub fn tnum_xor(a:Tnum, b:Tnum) -> Tnum {
    let v = a.value^b.value;
    let mu = a.mask|b.mask;

    Tnum::new(v&!mu,mu)
}

/// tnum 的乘法操作
pub fn tnum_mul(mut a:Tnum, mut b:Tnum) -> Tnum {
    let acc_v = a.value*b.value;
    let mut acc_m:Tnum = Tnum::new(0,0);
    while (a.value!=0)||(a.mask!=0) {
        if (a.value&1)!=0{
            acc_m = tnum_add(acc_m,Tnum::new(0,b.mask));
        }
        else if(a.mask&1)!=0 {
            acc_m = tnum_add(acc_m,Tnum::new(0,b.value|b.mask));
        }
        a = tnum_rshift(a,1);
        b = tnum_lshift(b,1);
    }
    tnum_add(Tnum::new(acc_v,0),acc_m)
}

/// tnum 的交集计算
pub fn tnum_intersect(a:Tnum, b:Tnum) -> Tnum {
    let v = a.value|b.value;
    let mu = a.mask&b.mask;
    Tnum::new(v&!mu, mu)
}

/// tnum 用与截断到指定字节大小
pub fn tnum_cast(mut a:Tnum,size:u8)->Tnum{
    //处理溢出
    a.value &= (1u64<<(size*8))-1;
    a.mask &= (1u64<<(size*8))-1;
    a
}

pub fn tnum_is_aligned(a:Tnum,size:u64)->bool{
    if size==0 {
        return true;
    }else{
        return ((a.value | a.mask) & (size - 1))==0;
    }
}

pub fn tnum_in(a:Tnum,mut b:Tnum)->bool{
    if (b.mask & !a.mask) != 0 {
        return false;
    }else{
        b.value &= !a.mask;
        return a.value == b.value;
    }
}

// tnum转换为字符串
pub fn tnum_sbin(size: usize, mut a: Tnum) -> String {
    let mut result = vec![0u8; size];
    
    // 从高位到低位处理每一位
    for n in (1..=64).rev() {
        if n < size {
            result[n - 1] = match (a.mask & 1, a.value & 1) {
                (1, _) => b'x',    // 不确定位
                (0, 1) => b'1',    // 确定位 1
                (0, 0) => b'0',    // 确定位 0
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
    String::from_utf8(result[..end].to_vec())
        .unwrap_or_else(|_| String::new())
}

pub fn tnum_subreg(a:Tnum)->Tnum{
    tnum_cast(a,4)
}

pub fn tnum_clear_subreg(a:Tnum)->Tnum{
    tnum_lshift(tnum_rshift(a, 32), 32)
}

pub fn tnum_with_subreg(reg:Tnum,subreg:Tnum)->Tnum{
    tnum_or(tnum_clear_subreg(reg),tnum_subreg(subreg))
}

pub fn tnum_const_subreg(a:Tnum,value:u32)->Tnum{
    tnum_with_subreg(a, tnum_const(value as u64))
}