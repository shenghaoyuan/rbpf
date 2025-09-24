//! Wrapped interval implementation for Solana eBPF
//! Based on the paper "A Wrapped Interval Arithmetic" by Jorge A. Navas et al.

use std::cmp::{max, min};

/// Trait for checking most significant bit (MSB) status
trait MsbCheck {
    fn is_msb_one(self, width: u32) -> bool;
    fn is_msb_zero(self, width: u32) -> bool;
    fn wrapping_div_signed(self, rhs: Self, width: u32) -> Self;
    fn wrapping_div_unsigned(self, rhs: Self, width: u32) -> Self;
}

/// 表示一个带位宽的环绕区间 [lb, ub]
#[derive(Debug, Clone, PartialEq)]
pub struct WrappedRange {
    /// 基础范围
    base: BaseRange,
    /// 是否为 bottom
    is_bottom: bool,
    /// widening 计数器
    counter_widening_cannot_doubling: u32,
}

/// 基础范围类型
#[derive(Debug, Clone, PartialEq)]
pub struct BaseRange {
    /// 变量标识符(可以为空)
    var: Option<String>,
    /// 状态改变计数
    num_of_changes: u32,
    /// 是否为格(lattice)
    is_lattice: bool,
    /// 是否为 top
    is_top: bool,
    /// 是否为 bottom
    is_bottom: bool,
    /// 下界
    lb: u64,
    /// 上界
    ub: u64,
    /// 位宽
    width: u32,
    /// 是否有符号
    is_signed: bool,
}

impl MsbCheck for u64 {
    fn is_msb_one(self, width: u32) -> bool {
        self & (1 << (width - 1)) != 0
    }

    fn is_msb_zero(self, width: u32) -> bool {
        self & (1 << (width - 1)) == 0
    }

    fn wrapping_div_signed(self, rhs: Self, width: u32) -> Self {
        let self_masked = self;
        let rhs_masked = rhs;

        // 检查除数是否为零
        if rhs_masked == 0 {
            panic!("wrapint: signed division by zero");
        }

        let dividend = get_signed_representation(self_masked, 64);
        let divisor = get_signed_representation(rhs_masked, 64);

        let result = dividend / divisor;

        result as u64
    }

    fn wrapping_div_unsigned(self, rhs: Self, width: u32) -> Self {
        let self_masked = self;
        let rhs_masked = rhs;

        // 检查除数是否为零
        if rhs_masked == 0 {
            panic!("wrapint: unsigned division by zero");
        }

        // 直接执行无符号除法
        self_masked / rhs_masked
    }
}

fn get_signed_representation(value: u64, width: u32) -> i64 {
    // 默认64位，简化计算
    let masked_value = value;

    if masked_value & (1u64 << 63) != 0 {
        // 负数：执行符号扩展
        // 直接使用二进制补码的符号扩展
        let sign_extended = masked_value | (!u64::MAX); // 将高位全部置1
        sign_extended as i64
    } else {
        // 正数：直接转换
        masked_value as i64
    }
}

impl BaseRange {
    fn new_constant(value: u64, width: u32, is_signed: bool) -> Self {
        Self {
            var: None,
            num_of_changes: 0,
            is_lattice: false,
            is_bottom: false,
            is_top: false,
            lb: value,
            ub: value,
            width,
            is_signed,
        }
    }

    fn new_bounds(lb: u64, ub: u64, width: u32, is_signed: bool) -> Self {
        Self {
            var: None,
            num_of_changes: 0,
            is_lattice: false,
            is_top: false,
            is_bottom: false,
            lb,
            ub,
            width,
            is_signed,
        }
    }
}

impl WrappedRange {
    /// 创建 bottom 值
    pub fn bottom(width: u32) -> Self {
        Self {
            base: BaseRange {
                var: None,
                num_of_changes: 0,
                is_lattice: false,
                is_top: false,
                is_bottom: true,
                lb: 0,
                ub: 0,
                width: 64,
                is_signed: false,
            },
            is_bottom: true,
            counter_widening_cannot_doubling: 0,
        }
    }

    /// 获取起始值
    pub fn get_start(&self) -> u64 {
        self.base.lb
    }

    /// 获取下界
    pub fn lb(&self) -> u64 {
        self.base.lb
    }

    /// 获取上界
    pub fn ub(&self) -> u64 {
        self.base.ub
    }

    /// 获取结束值
    pub fn get_end(&self) -> u64 {
        self.base.ub
    }

    /// 创建 top 值
    pub fn top(width: u32) -> Self {
        Self {
            base: BaseRange {
                var: None,
                num_of_changes: 0,
                is_lattice: true,
                is_top: true,
                is_bottom: false,
                lb: 0,
                ub: if width >= 64 {
                    u64::MAX
                } else {
                    (1u64 << width) - 1
                },
                width,
                is_signed: false,
            },
            is_bottom: false,
            counter_widening_cannot_doubling: 0,
        }
    }

    /// 从常量创建
    pub fn new_constant(value: u64, width: u32) -> Self {
        Self {
            base: BaseRange::new_constant(value, width, false),
            is_bottom: false,
            counter_widening_cannot_doubling: 0,
        }
    }

    /// 从上下界创建
    pub fn new_bounds(lb: u64, ub: u64, width: u32) -> Self {
        Self {
            base: BaseRange::new_bounds(lb, ub, width, false),
            is_bottom: false,
            counter_widening_cannot_doubling: 0,
        }
    }

    /// 获取有符号最大值 01111...1 (对应C++的get_signed_max)
    pub fn get_signed_max(width: u32) -> u64 {
        ((1u64 << (width - 1)) - 1)
    }

    /// 获取有符号最小值 1000....0 (对应C++的get_signed_min)
    pub fn get_signed_min(width: u32) -> u64 {
        1u64 << (width - 1)
    }

    /// 获取无符号最大值 1111....1 (对应C++的get_unsigned_max)
    pub fn get_unsigned_max(width: u32) -> u64 {
        match width {
            8 => 255,         // mod_8 - 1 = 256 - 1
            16 => 65535,      // mod_16 - 1 = 65536 - 1
            32 => 4294967295, // mod_32 - 1 = 4294967296 - 1
            64 => u64::MAX,
            _ => ((1u64 << width) - 1),
        }
    }

    /// 获取无符号最小值 0000....0 (对应C++的get_unsigned_min)
    pub fn get_unsigned_min(_width: u32) -> u64 {
        0
    }

    /// 检查是否为 bottom
    pub fn is_bottom(&self) -> bool {
        self.base.is_bottom
    }

    /// 检查是否为 top
    pub fn is_top(&self) -> bool {
        (!self.base.is_bottom
            && (self.base.ub.wrapping_sub(self.base.lb) == Self::get_unsigned_max(self.base.width)))
    }

    /// 获取位宽
    pub fn width(&self) -> u32 {
        self.base.width
    }

    /// 检查是否为常量区
    fn is_singleton(&self) -> bool {
        if self.is_bottom() {
            return false;
        }
        if self.is_top() {
            return false;
        }
        self.base.lb == self.base.ub
    }

    /// 设置为 bottom
    pub fn make_bottom(&mut self) {
        self.is_bottom = true;
        self.base.is_top = false;
    }

    /// 设置为 top
    pub fn make_top(&mut self) {
        self.base.is_top = true;
        self.is_bottom = false;
    }

    /// 重置 bottom 标志
    pub fn reset_bottom_flag(&mut self) {
        self.is_bottom = false;
    }

    /// 重置 top 标志
    pub fn reset_top_flag(&mut self) {
        self.base.is_top = false;
    }

    /// 字典序小于
    // fn lex_less_than(&self, x: u64, y: u64) -> bool {
    //     if self.is_msb_zero(x) && self.is_msb_one(y) {
    //         false
    //     } else if self.is_msb_one(x) && self.is_msb_zero(y) {
    //         true
    //     } else {
    //         x < y
    //     }
    // }

    // /// 字典序小于等于
    // fn lex_less_or_equal(&self, x: u64, y: u64) -> bool {
    //     self.lex_less_than(x, y) || x == y
    // }

    /// 计算基数(区间大小)
    pub fn cardinality(&self) -> u64 {
        if self.is_bottom {
            return 0;
        }

        if self.base.is_top {
            return if self.base.width >= 64 {
                u64::MAX
            } else {
                1u64 << self.base.width
            };
        }

        // 处理环绕情况
        if self.base.lb <= self.base.ub {
            self.base.ub - self.base.lb + 1
        } else {
            let max_val = if self.base.width >= 64 {
                u64::MAX
            } else {
                (1u64 << self.base.width) - 1
            };

            max_val
                .wrapping_sub(self.base.lb)
                .wrapping_add(self.base.ub)
                .wrapping_add(1)
        }
    }

    /// 检查是否为零区间
    // pub fn is_zero_range(&self) -> bool {
    //     !self.base.is_top && self.base.lb == 0 && self.base.ub == 0
    // }

    /// 检查给定值是否在区间内
    pub fn at(&self, value: u64) -> bool {
        if self.is_bottom() {
            return false;
        } else if self.is_top() {
            return true;
        }
        (value.wrapping_sub(self.base.lb)) <= (self.base.ub.wrapping_sub(self.base.lb))
    }

    /// 在北极点分割区间
    // pub fn nsplit(x: u64, y: u64, width: u32) -> Vec<Self> {
    //     // 创建北极点区间 [0111...1, 1000...0]
    //     let np_lb = (1u64 << (width - 1)) - 1; // 0111...1
    //     let np_ub = 1u64 << (width - 1); // 1000...0
    //     let np = Self::new_bounds(np_lb, np_ub, width);

    //     // 创建临时区间
    //     let s = Self::new_bounds(x, y, width);

    //     let mut res = Vec::new();

    //     // 如果不需要分割
    //     if !np.less_or_equal(&s) {
    //         res.push(s);
    //         return res;
    //     }

    //     // 分割成两个区间
    //     // [x, 0111...1]
    //     res.push(Self::new_bounds(x, np_lb, width));
    //     // [1000...0, y]
    //     res.push(Self::new_bounds(np_ub, y, width));

    //     res
    // }

    pub fn signed_split(&self, intervals: &mut Vec<WrappedRange>) {
        if self.is_bottom() {
            return;
        }

        let width = self.base.width;

        if self.is_top() {
            // Top情况：分割成两个最大的半球
            // [0, 2^(N-1)-1] 和 [2^(N-1), 2^N-1]
            let unsigned_min = Self::get_unsigned_min(width);
            let signed_max = Self::get_signed_max(width);
            let signed_min = Self::get_signed_min(width);
            let unsigned_max = Self::get_unsigned_max(width);

            // 第一个区间：[0, 正数最大值]
            intervals.push(WrappedRange::new_bounds(unsigned_min, signed_max, width));
            // 第二个区间：[负数最小值, 无符号最大值]
            intervals.push(WrappedRange::new_bounds(signed_min, unsigned_max, width));
        } else {
            // 检查是否跨越有符号边界
            let signed_limit = Self::new_bounds(
                Self::get_signed_max(width),
                Self::get_signed_min(width),
                width,
            );
            if signed_limit.less_or_equal(self) {
                let signed_max = Self::get_signed_max(width);
                let signed_min = Self::get_signed_min(width);

                // 分割成两个区间
                // [start, 正数最大值]
                intervals.push(WrappedRange::new_bounds(self.base.lb, signed_max, width));
                // [负数最小值, end]
                intervals.push(WrappedRange::new_bounds(signed_min, self.base.ub, width));
            } else {
                // 不跨越边界，直接添加自身
                intervals.push(self.clone());
            }
        }
    }

    pub fn unsigned_split(&self, intervals: &mut Vec<WrappedRange>) {
        if self.is_bottom() {
            return;
        }

        let width = self.base.width;

        if self.is_top() {
            // 为top情况，分割为两个最大区间
            intervals.push(Self::new_bounds(
                i64::MIN as u64,
                Self::get_unsigned_max(width),
                width,
            ));
            intervals.push(Self::new_bounds(
                Self::get_unsigned_min(width),
                i64::MAX as u64,
                width,
            ));
        } else {
            // 检查是否跨越无符号边界 (从最大值环绕到最小值)
            let unsigned_limit = Self::new_bounds(
                Self::get_unsigned_max(width),
                Self::get_unsigned_min(width),
                width,
            );
            if unsigned_limit.less_or_equal(self) {
                intervals.push(Self::new_bounds(
                    self.base.lb,
                    Self::get_unsigned_max(width),
                    width,
                ));
                intervals.push(Self::new_bounds(
                    Self::get_unsigned_min(width),
                    self.base.ub,
                    width,
                ));
            } else {
                intervals.push(self.clone());
            }
        }
    }

    pub fn signed_and_unsigned_split(&self, out: &mut Vec<WrappedRange>) {
        let mut ssplit = Vec::new();
        self.signed_split(&mut ssplit);

        for interval in ssplit {
            interval.unsigned_split(out);
        }
    }

    /// 从区间中去除零值，将其分割成不包含零的子区间
    pub fn trim_zero(&self, out: &mut Vec<WrappedRange>) {
        let width = self.width();
        let zero = 0u64;

        if !self.is_bottom() && !self.is_singleton_zero() {
            if self.base.lb == zero {
                // 如果起始点是零，返回 [1, end]
                out.push(WrappedRange::new_bounds(1, self.base.ub, width));
            } else if self.base.ub == zero {
                // 如果结束点是零，返回 [start, -1] (即 [start, u64::MAX])
                let minus_one = Self::get_unsigned_max(width);
                out.push(WrappedRange::new_bounds(self.base.lb, minus_one, width));
            } else if self.at(zero) {
                // 如果零在区间内部，分割成两个区间：[start, -1] 和 [1, end]
                let minus_one = Self::get_unsigned_max(width);
                out.push(WrappedRange::new_bounds(self.base.lb, minus_one, width));
                out.push(WrappedRange::new_bounds(1, self.base.ub, width));
            } else {
                // 如果零不在区间内，返回原区间
                out.push(self.clone());
            }
        }
    }

    /// 检查是否是只包含零的单例区间
    fn is_singleton_zero(&self) -> bool {
        !self.is_bottom() && self.base.lb == 0 && self.base.ub == 0
    }

    // // /// 在南北极点都分割区间
    // // pub fn psplit(x: u64, y: u64, width: u32) -> Vec<Self> {
    // //     let mut res = Vec::new();

    // //     // 先在北极点分割
    // //     let s1 = Self::nsplit(x, y, width);

    // //     // 对每个分割结果再在南极点分割
    // //     for r in s1.iter() {
    // //         let s2 = Self::signed_split(r.base.lb, r.base.ub, width);
    // //         // 将所有结果添加到结果集
    // //         res.extend(s2);
    // //     }

    // //     res
    // // }

    // /// 移除包含零的区间
    // pub fn purge_zero(r: &Self) -> Vec<Self> {
    //     let mut purged = Vec::new();

    //     assert!(!(r.base.lb == 0 && r.base.ub == 0), "区间不能为[0,0]");

    //     let width = r.base.width;
    //     let zero = Self::new_bounds(0, 0, width);

    //     if zero.less_or_equal(r) {
    //         if r.base.lb == 0 {
    //             if r.base.ub != 0 {
    //                 // 不跨越南极点的情况
    //                 purged.push(Self::new_bounds(
    //                     r.base.lb.wrapping_add(1),
    //                     r.base.ub,
    //                     width,
    //                 ));
    //             }
    //         } else {
    //             if r.base.ub == 0 {
    //                 // 区间如 [1000,0000]
    //                 let minus_one = if width >= 64 {
    //                     u64::MAX
    //                 } else {
    //                     (1u64 << width) - 1
    //                 };
    //                 purged.push(Self::new_bounds(r.base.lb, minus_one, width));
    //             } else {
    //                 // 跨越南极点的情况，分成两个区间
    //                 let minus_one = if width >= 64 {
    //                     u64::MAX
    //                 } else {
    //                     (1u64 << width) - 1
    //                 };
    //                 purged.push(Self::new_bounds(r.base.lb, minus_one, width));
    //                 purged.push(Self::new_bounds(1, r.base.ub, width));
    //             }
    //         }
    //     } else {
    //         // 不需要分割
    //         purged.push(r.clone());
    //     }

    //     purged
    // }

    // /// 批量移除包含零的区间
    // pub fn purge_zero_vec(vs: &[Self]) -> Vec<Self> {
    //     let mut res = Vec::new();
    //     for v in vs {
    //         let purged = Self::purge_zero(v);
    //         res.extend(purged);
    //     }
    //     res
    // }

    /// 检查是否小于等于
    pub fn less_or_equal(&self, x: &Self) -> bool {
        if x.is_top() || self.is_bottom() {
            return true;
        } else if x.is_bottom() || self.is_top() {
            return false;
        } else if self.base.lb == x.base.lb && self.base.ub == x.base.ub {
            return true;
        } else {
            return x.at(self.base.lb)
                && x.at(self.base.ub)
                && (!(self.at(x.base.lb)) || !(self.at(x.base.ub)));
        }
    }

    pub fn equal(&self, x: &Self) -> bool {
        self.less_or_equal(x) && x.less_or_equal(self)
    }

    // /// 计算环绕基数
    // fn w_card(x: u64, y: u64) -> u64 {
    //     if x <= y {
    //         y - x + 1
    //     } else {
    //         // 使用wrapping_add来避免溢出
    //         u64::MAX.wrapping_sub(x).wrapping_add(y).wrapping_add(1)
    //     }
    // }

    /// 环绕加法运算
    pub fn add(&self, x: &Self) -> Self {
        // 先检查bottom情况
        if self.is_bottom() || x.is_bottom() {
            return Self::bottom(64);
        }

        // 检查是否为top情况
        if self.is_top() || x.is_top() {
            return Self::top(64);
        }

        // [a,b] + [c,d] = [a+c,b+d] if no overflow
        // top           otherwise
        let x_sz = x.base.ub.wrapping_sub(x.base.lb);
        let sz = self.base.ub.wrapping_sub(self.base.lb);
        if x_sz.wrapping_add(sz).wrapping_add(1) <= x_sz {
            return Self::top(64);
        }
        Self::new_bounds(
            self.base.lb.wrapping_add(x.base.lb),
            self.base.ub.wrapping_add(x.base.ub),
            64,
        )
    }

    /// 环绕减法运算
    pub fn sub(&self, x: &Self) -> Self {
        // 先检查bottom情况
        if self.is_bottom() || x.is_bottom() {
            return Self::bottom(64);
        }

        // 检查是否为top情况
        if self.is_top() || x.is_top() {
            return Self::top(64);
        }
        // [a,b] - [c,d] = [a-d,b-c] if no overflow
        // top           otherwise
        let x_sz = x.base.ub.wrapping_sub(x.base.lb);
        let sz = self.base.ub.wrapping_sub(self.base.lb);
        if x_sz.wrapping_add(sz).wrapping_add(1) <= x_sz {
            return Self::top(64);
        }
        Self::new_bounds(
            self.base.lb.wrapping_sub(x.base.ub),
            self.base.ub.wrapping_sub(x.base.lb),
            64,
        )
    }

    // pub fn exact_meet(&self, x: &Self, out: &mut Vec<WrappedRange>){
    //     if self.is_bottom()||x.is_bottom(){
    //         return;
    //     }
    // }

    // /// 环绕乘法运算
    // pub fn mul(&mut self, x: &Self)-> Self {
    //     if self.is_bottom()||x.is_bottom(){
    //         return Self::bottom(64);
    //     }
    //     if self.is_top()||x.is_top(){
    //         return Self::top(64);
    //     }else{
    //         let mut cuts = Vec::<WrappedRange>::new();
    //         let mut x_cuts = Vec::<WrappedRange>::new();

    //         self.signed_and_unsigned_split(&mut cuts);
    //         x.signed_and_unsigned_split(&mut x_cuts);
    //         let mut res = Self::bottom(64);

    //         for cut in &cuts {
    //             for x_cut in &x_cuts {
    //                 let prod = cut.unsigned_mul(x_cut);
    //                 res = res.join(&prod);
    //             }
    //         }

    //         res
    //     }
    // }

    // /// 无符号除法
    // fn wrapped_unsigned_division(dividend: &Self, divisor: &Self) -> Self {
    //     let mut res = dividend.clone();

    //     let a = dividend.base.lb;
    //     let b = dividend.base.ub;
    //     let c = divisor.base.lb;
    //     let d = divisor.base.ub;

    //     res.base.lb = a.wrapping_div(d);
    //     res.base.ub = b.wrapping_div(c);

    //     res
    // }

    // /// 有符号除法
    // fn wrapped_signed_division(dividend: &Self, divisor: &Self) -> Self {
    //     let mut res = dividend.clone();

    //     // 将无符号值转换为有符号值
    //     let to_signed = |x: u64, width: u32| -> i64 {
    //         if x & (1 << (width - 1)) != 0 {
    //             -(((!x + 1) & ((1 << width) - 1)) as i64)
    //         } else {
    //             x as i64
    //         }
    //     };

    //     let from_signed = |x: i64, width: u32| -> u64 {
    //         if x < 0 {
    //             (!(-x as u64) + 1) & ((1 << width) - 1)
    //         } else {
    //             x as u64
    //         }
    //     };

    //     let width = dividend.base.width;
    //     let a = to_signed(dividend.base.lb, width);
    //     let b = to_signed(dividend.base.ub, width);
    //     let c = to_signed(divisor.base.lb, width);
    //     let d = to_signed(divisor.base.ub, width);

    //     let div1 = a.checked_div(d).map(|x| from_signed(x, width)).unwrap_or(0);
    //     let div2 = a.checked_div(c).map(|x| from_signed(x, width)).unwrap_or(0);
    //     let div3 = b.checked_div(d).map(|x| from_signed(x, width)).unwrap_or(0);
    //     let div4 = b.checked_div(c).map(|x| from_signed(x, width)).unwrap_or(0);

    //     res.base.lb = div1.min(div2).min(div3).min(div4);
    //     res.base.ub = div1.max(div2).max(div3).max(div4);

    //     res
    // }

    // /// 环绕除法运算
    // pub fn wrapped_division(&mut self, dividend: &Self, divisor: &Self, is_signed: bool) {
    //     // 处理特殊情况
    //     if dividend.is_zero_range() {
    //         self.base.lb = 0;
    //         self.base.ub = 0;
    //         return;
    //     }

    //     if divisor.is_zero_range() {
    //         self.make_bottom();
    //         return;
    //     }

    //     if is_signed {
    //         // 有符号除法
    //         let s1 = Self::psplit(dividend.base.lb, dividend.base.ub, dividend.base.width);
    //         let s2 = Self::purge_zero_vec(&Self::psplit(
    //             divisor.base.lb,
    //             divisor.base.ub,
    //             divisor.base.width,
    //         ));

    //         self.make_bottom();

    //         for i1 in s1.iter() {
    //             for i2 in s2.iter() {
    //                 let tmp = Self::wrapped_signed_division(i1, i2);
    //                 self.wrapped_join(&tmp);
    //             }
    //         }
    //     } else {
    //         // 无符号除法
    //         let s1 = Self::signed_split(dividend.base.lb, dividend.base.ub, dividend.base.width);
    //         let s2 = Self::purge_zero_vec(&Self::signed_split(
    //             divisor.base.lb,
    //             divisor.base.ub,
    //             divisor.base.width,
    //         ));

    //         self.make_bottom();

    //         for i1 in s1.iter() {
    //             for i2 in s2.iter() {
    //                 let tmp = Self::wrapped_unsigned_division(i1, i2);
    //                 self.wrapped_join(&tmp);
    //             }
    //         }
    //     }

    //     self.normalize();
    // }

    // /// 二元 Join 操作
    // pub fn wrapped_join(&mut self, other: &Self) {
    //     // 处理 bottom 情况
    //     if other.is_bottom {
    //         return;
    //     }
    //     if self.is_bottom {
    //         *self = other.clone();
    //         return;
    //     }

    //     // 处理 top 情况
    //     if other.is_top() || self.is_top() {
    //         self.make_top();
    //         return;
    //     }

    //     let a = self.base.lb;
    //     let b = self.base.ub;
    //     let c = other.base.lb;
    //     let d = other.base.ub;

    //     // 包含关系的情况
    //     if other.less_or_equal(self) {
    //         return;
    //     }
    //     if self.less_or_equal(other) {
    //         *self = other.clone();
    //         return;
    //     }

    //     // 一个覆盖另一个的情况
    //     if other.at(a) && other.at(b) && self.at(c) && self.at(d) {
    //         self.make_top();
    //         return;
    //     }

    //     // 重叠的情况
    //     if self.at(c) {
    //         self.base.lb = a;
    //         self.base.ub = d;
    //     } else if other.at(a) {
    //         self.base.lb = c;
    //         self.base.ub = b;
    //     }
    //     // 左/右倾斜的情况：非确定性情况
    //     // 这里使用字典序来解决平局
    //     else if Self::w_card(b, c) == Self::w_card(d, a) {
    //         if self.lex_less_than(a, c) {
    //             // 避免跨越北极点
    //             self.base.lb = a;
    //             self.base.ub = d;
    //         } else {
    //             // 避免跨越北极点
    //             self.base.lb = c;
    //             self.base.ub = b;
    //         }
    //     } else if Self::w_card(b, c) <= Self::w_card(d, a) {
    //         self.base.lb = a;
    //         self.base.ub = d;
    //     } else {
    //         self.base.lb = c;
    //         self.base.ub = b;
    //     }

    //     self.normalize_top();
    //     if !self.is_bottom && !other.is_bottom {
    //         self.reset_bottom_flag();
    //     }
    // }

    /// And 操作
    pub fn and(&self, x: &Self) -> Self {
        if self.less_or_equal(x) {
            return self.clone();
        } else if x.less_or_equal(self) {
            return x.clone();
        } else {
            let x_at_self_lb = x.at(self.base.lb);
            if x_at_self_lb {
                let self_at_x_lb = self.at(x.base.lb);
                if self_at_x_lb {
                    let span_a = self.base.ub.wrapping_sub(self.base.lb);
                    let span_b = x.base.ub.wrapping_sub(x.base.lb);
                    if (span_a < span_b) || (span_a == span_b && self.base.lb <= x.base.lb) {
                        return self.clone();
                    } else {
                        return x.clone();
                    }
                } else {
                    let x_at_self_ub = x.at(self.base.ub);
                    if x_at_self_ub {
                        return self.clone();
                    } else {
                        return Self::new_bounds(self.base.lb, x.base.ub, 64);
                    }
                }
            } else {
                let self_at_x_lb = self.at(x.base.lb);

                if self_at_x_lb {
                    let self_at_x_ub = self.at(x.base.ub);

                    if self_at_x_ub {
                        return x.clone();
                    } else {
                        return Self::new_bounds(x.base.lb, self.base.ub, 64);
                    }
                } else {
                    let result = Self::bottom(64);
                    return result;
                }
            }
        }
    }

    pub fn or(&self, x: &Self) -> Self {
        if self.less_or_equal(x) {
            return x.clone();
        } else if x.less_or_equal(self) {
            return self.clone();
        } else {
            if x.at(self.base.lb) && x.at(self.base.ub) && self.at(x.base.lb) && self.at(x.base.ub)
            {
                return Self::top(64);
            } else if x.at(self.base.ub) && self.at(x.base.lb) {
                return Self::new_bounds(self.base.lb, x.base.ub, 64);
            } else if self.at(x.base.ub) && x.at(self.base.lb) {
                return Self::new_bounds(x.base.lb, self.base.ub, 64);
            } else {
                let span_a = x.base.lb.wrapping_sub(self.base.ub);
                let span_b = self.base.lb.wrapping_sub(x.base.ub);
                if (span_a < span_b) || (span_a == span_b && self.base.lb <= x.base.lb) {
                    return Self::new_bounds(self.base.lb, x.base.ub, 64);
                } else {
                    return Self::new_bounds(x.base.lb, self.base.ub, 64);
                }
            }
        }
    }

    /// 无符号乘法
    pub fn unsigned_mul(&self, x: &Self) -> Self {
        assert!(!self.is_bottom() && !x.is_bottom());

        let width = self.base.width;
        let mut res = Self::top(width);

        // 检查乘法是否会溢出
        let m_start_bignum = self.base.lb as u128;
        let m_end_bignum = self.base.ub as u128;
        let x_start_bignum = x.base.lb as u128;
        let x_end_bignum = x.base.ub as u128;
        let unsigned_max = Self::get_unsigned_max(width) as u128;

        let prod1 = m_end_bignum * x_end_bignum;
        let prod2 = m_start_bignum * x_start_bignum;
        let diff = if prod1 > prod2 {
            prod1 - prod2
        } else {
            prod2 - prod1
        };

        if diff < unsigned_max {
            res = Self::new_bounds(
                self.base.lb.wrapping_mul(x.base.lb),
                self.base.ub.wrapping_mul(x.base.ub),
                width,
            );
        }

        res
    }

    /// 精确交集运算
    /// 如果 out 为空，则交集为空
    pub fn exact_meet(&self, x: &Self, out: &mut Vec<WrappedRange>) {
        if self.is_bottom() || x.is_bottom() {
            // bottom
            return;
        } else if *self == *x || self.is_top() {
            out.push(x.clone());
        } else if x.is_top() {
            out.push(self.clone());
        } else if x.at(self.base.lb)
            && x.at(self.base.ub)
            && self.at(x.base.lb)
            && self.at(x.base.ub)
        {
            out.push(Self::new_bounds(self.base.lb, x.base.ub, self.base.width));
            out.push(Self::new_bounds(x.base.lb, self.base.ub, self.base.width));
        } else if x.at(self.base.lb) && x.at(self.base.ub) {
            out.push(self.clone());
        } else if self.at(x.base.lb) && self.at(x.base.ub) {
            out.push(x.clone());
        } else if x.at(self.base.lb)
            && x.at(self.base.ub)
            && !x.at(self.base.ub)
            && self.at(x.base.lb)
        {
            out.push(Self::new_bounds(self.base.lb, x.base.ub, self.base.width));
        } else if x.at(self.base.ub)
            && self.at(x.base.lb)
            && !x.at(self.base.lb)
            && self.at(x.base.ub)
        {
            out.push(Self::new_bounds(x.base.lb, self.base.ub, self.base.width));
        } else {
            // bottom - out 保持为空
        }
    }

    pub fn reduced_signed_unsigned_mul(&self, x: &Self, out: &mut Vec<WrappedRange>) {
        if self.is_bottom() || x.is_bottom() {
            return;
        }

        let s = self.signed_mul(x);
        let u = self.unsigned_mul(x);
        s.exact_meet(&u, out);
    }

    /// 有符号乘法
    pub fn signed_mul(&self, x: &Self) -> Self {
        assert!(!self.is_bottom() && !x.is_bottom());

        let width = self.base.width;
        let msb_start = self.base.lb.is_msb_one(width);
        let msb_end = self.base.ub.is_msb_one(width);
        let msb_x_start = x.base.lb.is_msb_one(width);
        let msb_x_end = x.base.ub.is_msb_one(width);

        let mut res = Self::top(width);

        if msb_start == msb_end && msb_end == msb_x_start && msb_x_start == msb_x_end {
            // 两个区间在同一半球
            if !msb_start {
                return self.unsigned_mul(x);
            } else {
                // 检查乘法是否会溢出
                let m_start_bignum = self.base.lb as u128;
                let m_end_bignum = self.base.ub as u128;
                let x_start_bignum = x.base.lb as u128;
                let x_end_bignum = x.base.ub as u128;
                let unsigned_max = Self::get_unsigned_max(width) as u128;

                let prod1 = m_start_bignum * x_start_bignum;
                let prod2 = m_end_bignum * x_end_bignum;
                let diff = if prod1 > prod2 {
                    prod1 - prod2
                } else {
                    prod2 - prod1
                };

                if diff < unsigned_max {
                    res = Self::new_bounds(
                        self.base.ub.wrapping_mul(x.base.ub),
                        self.base.lb.wrapping_mul(x.base.lb),
                        width,
                    );
                }
                return res;
            }
        }

        // 每个区间不能跨越边界：一个区间在不同的半球
        if !(msb_start != msb_end || msb_x_start != msb_x_end) {
            if msb_start && !msb_x_start {
                // 检查乘法是否会溢出
                let m_start_bignum = self.base.lb as u128;
                let m_end_bignum = self.base.ub as u128;
                let x_start_bignum = x.base.lb as u128;
                let x_end_bignum = x.base.ub as u128;
                let unsigned_max = Self::get_unsigned_max(width) as u128;

                let mul1 = m_end_bignum * x_start_bignum;
                let mul2 = m_start_bignum * x_end_bignum;
                if mul1 >= mul2 && mul1 - mul2 < unsigned_max {
                    res = Self::new_bounds(
                        self.base.lb.wrapping_mul(x.base.ub),
                        self.base.ub.wrapping_mul(x.base.lb),
                        width,
                    );
                }
            } else if !msb_start && msb_x_start {
                // 检查乘法是否会溢出
                let m_start_bignum = self.base.lb as u128;
                let m_end_bignum = self.base.ub as u128;
                let x_start_bignum = x.base.lb as u128;
                let x_end_bignum = x.base.ub as u128;
                let unsigned_max = Self::get_unsigned_max(width) as u128;

                let mul1 = m_start_bignum * x_end_bignum;
                let mul2 = m_end_bignum * x_start_bignum;
                if mul1 >= mul2 && mul1 - mul2 < unsigned_max {
                    res = Self::new_bounds(
                        self.base.ub.wrapping_mul(x.base.lb),
                        self.base.lb.wrapping_mul(x.base.ub),
                        width,
                    );
                }
            }
        }

        res
    }

    /// 乘法运算
    pub fn mul(&self, x: &Self) -> Self {
        if self.is_bottom() || x.is_bottom() {
            return Self::bottom(self.base.width);
        }
        if self.is_top() || x.is_top() {
            return Self::top(self.base.width);
        } else {
            let mut cuts = Vec::new();
            let mut x_cuts = Vec::new();

            self.signed_and_unsigned_split(&mut cuts);
            x.signed_and_unsigned_split(&mut x_cuts);

            assert!(!cuts.is_empty());
            assert!(!x_cuts.is_empty());

            let mut res = Self::bottom(self.base.width);
            for cut in &cuts {
                for x_cut in &x_cuts {
                    let mut exact_reduct = Vec::new();
                    cut.reduced_signed_unsigned_mul(x_cut, &mut exact_reduct);
                    for interval in exact_reduct {
                        res = res.or(&interval);
                    }
                }
            }

            res
        }
    }

    /// 有符号除法
    pub fn signed_div(&self, x: &Self) -> Self {
        assert!(!x.at(0));
        assert!(!self.is_bottom() && !x.is_bottom());

        let width = self.width();
        let msb_start = self.base.lb.is_msb_one(width);
        let msb_x_start = x.base.lb.is_msb_one(width);

        let smin = Self::get_signed_min(width);
        let minus_one = Self::get_unsigned_max(width); // -1 in two's complement

        let mut res = Self::top(width);

        if msb_start == msb_x_start {
            if msb_start {
                // 两个都是负数
                // 检查除法是否会溢出
                if !((self.base.ub == smin && x.base.lb == minus_one)
                    || (self.base.lb == smin && x.base.ub == minus_one))
                {
                    res = Self::new_bounds(
                        self.base.ub.wrapping_div_signed(x.base.lb, width),
                        self.base.lb.wrapping_div_signed(x.base.ub, width),
                        width,
                    );
                }
            } else {
                // 两个都是正数
                // 检查除法是否会溢出
                if !((self.base.lb == smin && x.base.ub == minus_one)
                    || (self.base.ub == smin && x.base.lb == minus_one))
                {
                    res = Self::new_bounds(
                        self.base.lb.wrapping_div_signed(x.base.ub, width),
                        self.base.ub.wrapping_div_signed(x.base.lb, width),
                        width,
                    );
                }
            }
        } else {
            if msb_start {
                // self是负数，x是正数
                // 检查除法是否会溢出
                if !((self.base.lb == smin && x.base.lb == minus_one)
                    || (self.base.ub == smin && x.base.ub == minus_one))
                {
                    res = Self::new_bounds(
                        self.base.lb.wrapping_div_signed(x.base.lb, width),
                        self.base.ub.wrapping_div_signed(x.base.ub, width),
                        width,
                    );
                }
            } else {
                // self是正数，x是负数
                // 检查除法是否会溢出
                if !((self.base.ub == smin && x.base.ub == minus_one)
                    || (self.base.lb == smin && x.base.lb == minus_one))
                {
                    res = Self::new_bounds(
                        self.base.ub.wrapping_div_signed(x.base.ub, width),
                        self.base.lb.wrapping_div_signed(x.base.lb, width),
                        width,
                    );
                }
            }
        }

        res
    }

    /// 有符号除法操作
    pub fn sdiv(&self, x: &Self) -> Self {
        if self.is_bottom() || x.is_bottom() {
            return Self::bottom(self.width());
        }
        if self.is_top() || x.is_top() {
            return Self::top(self.width());
        } else {
            let mut cuts = Vec::new();
            let mut x_cuts = Vec::new();

            self.signed_and_unsigned_split(&mut cuts);
            x.signed_and_unsigned_split(&mut x_cuts);

            assert!(!cuts.is_empty());
            assert!(!x_cuts.is_empty());

            let mut res = Self::bottom(self.width());
            for cut in &cuts {
                for x_cut in &x_cuts {
                    let mut trimmed_divisors = Vec::new();
                    x_cut.trim_zero(&mut trimmed_divisors);
                    for divisor in trimmed_divisors {
                        res = res.or(&cut.signed_div(&divisor));
                    }
                }
            }

            res
        }
    }

    pub fn unsigned_div(&self, x: &Self) -> Self {
        assert!(!x.at(0));
        assert!(!self.is_bottom() && !x.is_bottom());
        let res = Self::new_bounds(
            self.base.lb.wrapping_div_unsigned(x.base.ub, self.width()),
            self.base.ub.wrapping_div_unsigned(x.base.lb, self.width()),
            self.width(),
        );
        res
    }

    /// 无符号除法操作
    pub fn udiv(&self, x: &Self) -> Self {
        if self.is_bottom() || x.is_bottom() {
            return Self::bottom(self.width());
        }
        if self.is_top() || x.is_top() {
            return Self::top(self.width());
        } else {
            let mut cuts = Vec::new();
            let mut x_cuts = Vec::new();

            self.signed_split(&mut cuts);
            x.signed_split(&mut x_cuts);

            assert!(!cuts.is_empty());
            assert!(!x_cuts.is_empty());

            let mut res = Self::bottom(self.width());
            for cut in &cuts {
                for x_cut in &x_cuts {
                    let mut trimmed_divisors = Vec::new();
                    x_cut.trim_zero(&mut trimmed_divisors);
                    for divisor in trimmed_divisors {
                        res = res.or(&cut.unsigned_div(&divisor));
                    }
                }
            }

            res
        }
    }

    pub fn default_implementation(&self, x: &Self) -> Self {
        if self.is_bottom() || x.is_bottom() {
            return Self::bottom(self.width());
        } else {
            return Self::top(self.width());
        }
    }

    pub fn urem(&self, x: &Self) -> Self {
        self.default_implementation(x)
    }

    pub fn srem(&self, x: &Self) -> Self {
        self.default_implementation(x)
    }

    /// 截断函数
    pub fn trunc(&self, bits_to_keep: u32) -> Self {
        if self.is_bottom() || self.is_top() {
            return self.clone();
        }

        let w = self.width();

        // 检查高位部分是否相同
        let start_high = self.lb() >> bits_to_keep;
        let end_high = self.ub() >> bits_to_keep;

        if start_high == end_high {
            // 高位相同，可以安全截断
            let lower_start = self.keep_lower(bits_to_keep);
            let lower_end = self.ub() & ((1u64 << bits_to_keep) - 1);

            if lower_start <= lower_end {
                return Self::new_bounds(lower_start, lower_end, bits_to_keep);
            }
        } else {
            // 高位不同，需要检查环绕情况
            let y = start_high + 1;
            if y == end_high {
                let lower_start = self.keep_lower(bits_to_keep);
                let lower_end = self.ub() & ((1u64 << bits_to_keep) - 1);

                if !(lower_start <= lower_end) {
                    return Self::new_bounds(lower_start, lower_end, bits_to_keep);
                }
            }
        }

        // 如果无法确定，返回top
        Self::top(bits_to_keep)
    }

    /// 保留低位bits_to_keep位的辅助函数
    fn keep_lower(&self, bits_to_keep: u32) -> u64 {
        if bits_to_keep >= self.width() {
            return self.lb();
        }

        let mask = (1u64 << bits_to_keep) - 1;
        self.lb() & mask
    }

    // // 常量左移
    // pub fn shl_const(&self, k: u64) -> Self {
    //     if self.is_bottom() {
    //         return self.clone();
    //     }

    //     if self.is_top() {
    //         return self.clone();
    //     }

    //     let b = self.width();
    //     let truncated = self.trunc(b - k as u32);

    //     if !truncated.is_top() {
    //         let start = self.lb() << k;
    //         let end = self.ub() << k;
    //         Self::new_bounds(start, end, b)
    //     } else {
    //         Self::top(b)
    //     }
    // }

    // // 区间左移
    // pub fn shl(&self, x: &Self) -> Self {
    //     if self.is_bottom() {
    //         return self.clone();
    //     }

    //     // 只有当位移量是单例时才执行
    //     if x.is_singleton() {
    //         self.shl_const(x.lb())
    //     } else {
    //         Self::top(self.width())
    //     }
    // }

    // Widening 操作
    // pub fn widening(&mut self, previous_v: &Self, jump_set: &[i64]) {
    //     if previous_v.is_bottom {
    //         return;
    //     }

    //     let old = previous_v.clone();

    //     let u = old.base.lb;
    //     let v = old.base.ub;
    //     let x = self.base.lb;
    //     let y = self.base.ub;

    //     let mut can_doubling_interval = true;
    //     let card_old = Self::w_card(u, v);

    //     // 溢出检查
    //     if Self::check_overflow_for_widening_jump(card_old, self.base.width) {
    //         self.counter_widening_cannot_doubling += 1;
    //         if self.counter_widening_cannot_doubling < 5 {
    //             can_doubling_interval = false;
    //         } else {
    //             self.make_top();
    //             self.counter_widening_cannot_doubling = 0;
    //             return;
    //         }
    //     }

    //     let mut merged = old.clone();
    //     merged.wrapped_join(self);

    //     let width = x.count_zeros() as u32;
    //     if old.less_or_equal(self) && !old.at(x) && !old.at(y) {
    //         if !can_doubling_interval {
    //             let mut widen_lb = x;
    //             let mut widen_ub = x.wrapping_add(card_old).wrapping_add(card_old);

    //             let mut jump_lb = 0;
    //             let mut jump_ub = 0;
    //             Self::widen_one_interval(
    //                 merged.base.lb,
    //                 merged.base.ub,
    //                 width,
    //                 jump_set,
    //                 &mut jump_lb,
    //                 &mut jump_ub,
    //             );

    //             {
    //                 let tmp = Self::make_smaller_interval(merged.base.lb, widen_lb, width);
    //                 if tmp.at(jump_lb) {
    //                     widen_lb = jump_lb;
    //                 }
    //             }
    //             {
    //                 let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
    //                 if tmp.at(jump_ub) {
    //                     widen_ub = jump_ub;
    //                 }
    //             }

    //             self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
    //             let tmp = Self::new_bounds(x, y, width);
    //             self.wrapped_join(&tmp);
    //         } else {
    //             let mut widen_lb = x;
    //             let mut widen_ub = x.wrapping_add(card_old).wrapping_add(card_old);

    //             let mut jump_lb = 0;
    //             let mut jump_ub = 0;
    //             Self::widen_one_interval(
    //                 merged.base.lb,
    //                 merged.base.ub,
    //                 width,
    //                 jump_set,
    //                 &mut jump_lb,
    //                 &mut jump_ub,
    //             );

    //             {
    //                 let tmp = Self::make_smaller_interval(merged.base.lb, widen_lb, width);
    //                 if tmp.at(jump_lb) {
    //                     widen_lb = jump_lb;
    //                 }
    //             }
    //             {
    //                 let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
    //                 if tmp.at(jump_ub) {
    //                     widen_ub = jump_ub;
    //                 }
    //             }

    //             self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
    //             let tmp = Self::new_bounds(x, y, width);
    //             self.wrapped_join(&tmp);
    //         }
    //     } else if merged.base.lb == u && merged.base.ub == y {
    //         if !can_doubling_interval {
    //             let mut widen_lb = u;
    //             let mut widen_ub = u.wrapping_add(card_old).wrapping_add(card_old);

    //             let mut jump_lb__ = 0;
    //             let mut jump_ub = 0;
    //             Self::widen_one_interval(
    //                 merged.base.lb,
    //                 merged.base.ub,
    //                 width,
    //                 jump_set,
    //                 &mut jump_lb__,
    //                 &mut jump_ub,
    //             );

    //             {
    //                 let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
    //                 if tmp.at(jump_ub) {
    //                     widen_ub = jump_ub;
    //                 }
    //             }

    //             self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
    //             let tmp = Self::new_bounds(u, y, width);
    //             self.wrapped_join(&tmp);
    //         } else {
    //             let mut widen_lb = u;
    //             let mut widen_ub = u.wrapping_add(card_old).wrapping_add(card_old);

    //             let mut jump_lb__ = 0;
    //             let mut jump_ub = 0;
    //             Self::widen_one_interval(
    //                 merged.base.lb,
    //                 merged.base.ub,
    //                 width,
    //                 jump_set,
    //                 &mut jump_lb__,
    //                 &mut jump_ub,
    //             );

    //             {
    //                 let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
    //                 if tmp.at(jump_ub) {
    //                     widen_ub = jump_ub;
    //                 }
    //             }

    //             self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
    //             let tmp = Self::new_bounds(u, y, width);
    //             self.wrapped_join(&tmp);
    //         }
    //     } else if merged.base.lb == x && merged.base.ub == v {
    //         if !can_doubling_interval {
    //             let mut widen_lb = 0;
    //             let mut widen_ub = v;
    //             let mut widen_ub__ = 0;
    //             Self::widen_one_interval(
    //                 merged.base.lb,
    //                 merged.base.ub,
    //                 width,
    //                 jump_set,
    //                 &mut widen_lb,
    //                 &mut widen_ub__,
    //             );
    //             self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
    //             let tmp = Self::new_bounds(x, y, width);
    //             self.wrapped_join(&tmp);
    //         } else {
    //             let mut widen_lb = u.wrapping_sub(card_old).wrapping_sub(card_old);
    //             let mut widen_ub = v;

    //             let mut jump_lb = 0;
    //             let mut jump_ub__ = 0;
    //             Self::widen_one_interval(
    //                 merged.base.lb,
    //                 merged.base.ub,
    //                 width,
    //                 jump_set,
    //                 &mut jump_lb,
    //                 &mut jump_ub__,
    //             );

    //             {
    //                 let tmp = Self::make_smaller_interval(merged.base.lb, widen_lb, width);
    //                 if tmp.at(jump_lb) {
    //                     widen_lb = jump_lb;
    //                 }
    //             }

    //             self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
    //             let tmp = Self::new_bounds(x, v, width);
    //             self.wrapped_join(&tmp);
    //         }
    //     } else {
    //         // 否则，返回旧区间
    //         self.base.lb = old.base.lb;
    //         self.base.ub = old.base.ub;
    //     }

    //     self.normalize_top();
    // }

    // /// 泛化的 Join 操作
    // pub fn generalized_join(&mut self, values: Vec<&Self>) {
    //     if values.is_empty() {
    //         self.make_bottom();
    //         return;
    //     }

    //     // 按照左界的字典序排序
    //     let mut sorted_values = values.clone();
    //     sorted_values.sort_by(|a, b| {
    //         if self.lex_less_or_equal(a.base.lb, b.base.lb) {
    //             std::cmp::Ordering::Less
    //         } else {
    //             std::cmp::Ordering::Greater
    //         }
    //     });

    //     let mut f = self.clone();
    //     f.make_bottom();

    //     // 处理跨越南极点的情况
    //     for v in sorted_values.iter() {
    //         if v.is_top() || Self::cross_south_pole(v.base.lb, v.base.ub) {
    //             f = Self::extend(&f, v);
    //         }
    //     }

    //     let mut g = self.clone();
    //     g.make_bottom();

    //     for v in sorted_values.iter() {
    //         let tmp = Self::clock_wise_gap(&f, v);
    //         g = Self::bigger(&g, &tmp);
    //         f = Self::extend(&f, v);
    //     }

    //     let tmp = Self::wrapped_complement(&Self::bigger(&g, &Self::wrapped_complement(&f)));
    //     self.base.lb = tmp.base.lb;
    //     self.base.ub = tmp.base.ub;
    // }

    // /// 辅助函数：检查是否跨越南极点
    // fn cross_south_pole(x: u64, y: u64) -> bool {
    //     y < x
    // }

    // /// 辅助函数：扩展区间
    // fn extend(r1: &Self, r2: &Self) -> Self {
    //     let mut res = r1.clone();
    //     let mut tmp = r2.clone();
    //     res.wrapped_join(&tmp);
    //     res
    // }

    // /// 辅助函数：选择更大的区间
    // fn bigger(r1: &Self, r2: &Self) -> Self {
    //     if r1.is_bottom() && !r2.is_bottom() {
    //         return r2.clone();
    //     }
    //     if r2.is_bottom() && !r1.is_bottom() {
    //         return r1.clone();
    //     }
    //     if r2.is_bottom() && r1.is_bottom() {
    //         return r1.clone();
    //     }

    //     if Self::lex_less_or_equal_static(
    //         Self::w_card(r2.base.lb, r2.base.ub),
    //         Self::w_card(r1.base.lb, r1.base.ub),
    //     ) {
    //         r1.clone()
    //     } else {
    //         r2.clone()
    //     }
    // }

    // /// 辅助函数：计算顺时针间隔
    // fn clock_wise_gap(r1: &Self, r2: &Self) -> Self {
    //     let mut gap = Self::new_bounds(
    //         r1.base.ub.wrapping_add(1),
    //         r2.base.lb.wrapping_sub(1),
    //         r1.base.width,
    //     );

    //     if r1.is_bottom() || r2.is_bottom() || r2.at(r1.base.ub) || r1.at(r2.base.lb) {
    //         gap.make_bottom();
    //     }

    //     gap
    // }

    // /// 辅助函数：计算补集
    // fn wrapped_complement(r: &Self) -> Self {
    //     let mut c = r.clone();

    //     if r.is_bottom() {
    //         c.make_top();
    //         return c;
    //     }
    //     if r.is_top() {
    //         c.make_bottom();
    //         return c;
    //     }

    //     c.base.lb = r.base.ub.wrapping_add(1);
    //     c.base.ub = r.base.lb.wrapping_sub(1);
    //     c
    // }

    // /// 逻辑位运算
    // pub fn wrapped_logical_bitwise(&mut self, op1: &Self, op2: &Self, op_code: u32) {
    //     // 重置状态
    //     self.reset_bottom_flag();
    //     self.reset_top_flag();

    //     // 如果任一操作数为 bottom，结果为 bottom
    //     if op1.is_bottom || op2.is_bottom {
    //         self.make_bottom();
    //         return;
    //     }

    //     // 如果任一操作数为 top，结果为 top
    //     if op1.is_top() || op2.is_top() {
    //         self.make_top();
    //         return;
    //     }

    //     match op_code {
    //         // AND
    //         0 => {
    //             self.base.lb = op1.base.lb & op2.base.lb;
    //             self.base.ub = op1.base.ub & op2.base.ub;
    //         }
    //         // OR
    //         1 => {
    //             self.base.lb = op1.base.lb | op2.base.lb;
    //             self.base.ub = op1.base.ub | op2.base.ub;
    //         }
    //         // XOR
    //         2 => {
    //             self.base.lb = op1.base.lb ^ op2.base.lb;
    //             self.base.ub = op1.base.ub ^ op2.base.ub;
    //         }
    //         _ => {}
    //     }

    //     // 保持位宽不变
    //     self.base.width = op1.base.width;
    //     self.normalize();
    // }

    // /// 位移运算
    // pub fn wrapped_bitwise_shifts(&mut self, op1: &Self, op2: &Self, op_code: u32) {
    //     // 重置状态
    //     self.reset_bottom_flag();
    //     self.reset_top_flag();

    //     // 如果任一操作数为 bottom，结果为 bottom
    //     if op1.is_bottom || op2.is_bottom {
    //         self.make_bottom();
    //         return;
    //     }

    //     // 如果任一操作数为 top，结果为 top
    //     if op1.is_top() || op2.is_top() {
    //         self.make_top();
    //         return;
    //     }

    //     // 计算位移后保留的位数
    //     let num_bits_survive_shift = op1.base.width as i64 - op2.base.ub as i64;
    //     if num_bits_survive_shift <= 0 {
    //         self.base.lb = 0;
    //         self.base.ub = 0;
    //         self.base.width = op1.base.width;
    //         return;
    //     }

    //     match op_code {
    //         // SHL
    //         0 => {
    //             self.base.lb = op1.base.lb << op2.base.lb;
    //             self.base.ub = op1.base.ub << op2.base.ub;
    //         }
    //         // LSHR (逻辑右移)
    //         1 => {
    //             let mask = (1 << num_bits_survive_shift) - 1;
    //             self.base.lb = (op1.base.lb >> op2.base.lb) & mask;
    //             self.base.ub = (op1.base.ub >> op2.base.ub) & mask;
    //         }
    //         // ASHR (算术右移)
    //         2 => {
    //             let sign_bit = 1 << (op1.base.width - 1);
    //             let is_negative = (op1.base.lb & sign_bit) != 0;
    //             let mask = (1 << num_bits_survive_shift) - 1;

    //             if is_negative {
    //                 self.base.lb =
    //                     ((op1.base.lb >> op2.base.lb) | !mask) & ((1 << op1.base.width) - 1);
    //                 self.base.ub =
    //                     ((op1.base.ub >> op2.base.ub) | !mask) & ((1 << op1.base.width) - 1);
    //             } else {
    //                 self.base.lb = (op1.base.lb >> op2.base.lb) & mask;
    //                 self.base.ub = (op1.base.ub >> op2.base.ub) & mask;
    //             }
    //         }
    //         _ => {}
    //     }

    //     // 保持位宽不变
    //     self.base.width = op1.base.width;
    //     self.normalize();
    // }

    // /// 取模运算
    // pub fn wrapped_rem(&mut self, dividend: &Self, divisor: &Self, is_signed_rem: bool) {
    //     // 处理特殊情况
    //     if dividend.is_zero_range() {
    //         self.base.lb = 0;
    //         self.base.ub = 0;
    //         return;
    //     }
    //     if divisor.is_zero_range() {
    //         self.make_bottom();
    //         return;
    //     }

    //     if is_signed_rem {
    //         let s1 = Self::signed_split(dividend.base.lb, dividend.base.ub, dividend.base.width);
    //         let s2 = Self::purge_zero_vec(&Self::signed_split(
    //             divisor.base.lb,
    //             divisor.base.ub,
    //             divisor.base.width,
    //         ));

    //         // 确保除数不为空（不应该包含区间[0,0]）
    //         assert!(!s2.is_empty(), "Sanity check: empty means interval [0,0]");

    //         self.make_bottom();

    //         for i1 in s1.iter() {
    //             for i2 in s2.iter() {
    //                 let a = i1.base.lb;
    //                 let b = i1.base.ub;
    //                 let c = i2.base.lb;
    //                 let d = i2.base.ub;

    //                 let is_zero_a = !self.is_msb_one(a);
    //                 let is_zero_c = !self.is_msb_one(c);

    //                 let width = self.base.width;
    //                 let (lb, ub) = if is_zero_a && is_zero_c {
    //                     // [0,d-1]
    //                     (0, d.wrapping_sub(1))
    //                 } else if is_zero_a && !is_zero_c {
    //                     // [0,-c-1]
    //                     (0, c.wrapping_neg().wrapping_sub(1))
    //                 } else if !is_zero_a && is_zero_c {
    //                     // [-d+1,0]
    //                     (d.wrapping_sub(1).wrapping_neg(), 0)
    //                 } else if !is_zero_a && !is_zero_c {
    //                     // [c+1,0]
    //                     (c.wrapping_add(1), 0)
    //                 } else {
    //                     unreachable!("This should be unreachable!");
    //                 };

    //                 let mut tmp = Self::new_bounds(lb, ub, width);
    //                 self.wrapped_join(&tmp);
    //             }
    //         }
    //     } else {
    //         // 无符号取模：在南极点分割并计算每个笛卡尔积元素的无符号操作
    //         let s1 = Self::signed_split(dividend.base.lb, dividend.base.ub, dividend.base.width);
    //         let s2 = Self::purge_zero_vec(&Self::signed_split(
    //             divisor.base.lb,
    //             divisor.base.ub,
    //             divisor.base.width,
    //         ));

    //         // 确保除数不为空（不应该包含区间[0,0]）
    //         assert!(!s2.is_empty(), "Sanity check: empty means interval [0,0]");

    //         self.make_bottom();

    //         for i1 in s1.iter() {
    //             for i2 in s2.iter() {
    //                 let d = i2.base.ub;
    //                 let lb = 0;
    //                 let ub = d.wrapping_sub(1);
    //                 let mut tmp = Self::new_bounds(lb, ub, self.base.width);
    //                 self.wrapped_join(&tmp);
    //                 // }
    //             }
    //         }
    //     }

    //     self.normalize();
    // }

    // /// 同半球有符号小于等于比较
    // fn comparison_sle_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
    //     i1.base.lb <= i2.base.ub
    // }

    // /// 同半球有符号小于比较
    // fn comparison_slt_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
    //     i1.base.lb < i2.base.ub
    // }

    // /// 同半球无符号小于等于比较
    // fn comparison_ule_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
    //     i1.base.lb <= i2.base.ub
    // }

    // /// 同半球无符号小于比较
    // fn comparison_ult_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
    //     i1.base.lb < i2.base.ub
    // }

    // /// 规范化区间
    // fn normalize(&mut self) {
    //     // 如果是 bottom 或 top，不需要规范化
    //     if self.is_bottom || self.base.is_top {
    //         return;
    //     }

    //     // 检查是否需要设置为 top
    //     let max_val = if self.base.width >= 64 {
    //         u64::MAX
    //     } else {
    //         (1u64 << self.base.width) - 1
    //     };

    //     if self.base.lb > max_val || self.base.ub > max_val {
    //         self.make_top();
    //         return;
    //     }

    //     // 规范化上下界
    //     self.base.lb &= max_val;
    //     self.base.ub &= max_val;
    // }

    // /// 规范化为 top (对应 C++ 的 normalizeTop)
    // fn normalize_top(&mut self) {
    //     // 如果是 bottom，不需要规范化
    //     if self.is_bottom {
    //         return;
    //     }

    //     // 检查是否需要设置为 top
    //     let max_val = if self.base.width >= 64 {
    //         u64::MAX
    //     } else {
    //         (1u64 << self.base.width) - 1
    //     };

    //     if self.base.lb == 0 && self.base.ub == max_val {
    //         self.make_top();
    //     }
    // }

    // fn check_overflow_for_widening_jump(card: u64, width: u32) -> bool {
    //     // 计算最大值 2^(w-1)
    //     let max = if width <= 1 { 0 } else { 1u64 << (width - 1) };
    //     card >= max
    // }

    // fn widen_one_interval(
    //     a: u64,
    //     b: u64,
    //     width: u32,
    //     jump_set: &[i64],
    //     lb: &mut u64,
    //     ub: &mut u64,
    // ) {
    //     // 初始化为最小值和最大值
    //     *lb = if width >= 64 { u64::MIN } else { 0 };
    //     *ub = if width >= 64 {
    //         u64::MAX
    //     } else {
    //         (1u64 << width) - 1
    //     };

    //     let mut first_lb = true;
    //     let mut first_ub = true;

    //     for &landmark in jump_set {
    //         let landmark_u64 = landmark as u64;
    //         if Self::lex_less_or_equal(landmark_u64, a) {
    //             if first_lb {
    //                 *lb = landmark_u64;
    //                 first_lb = false;
    //             } else {
    //                 *lb = (*lb).max(landmark_u64);
    //             }
    //         }
    //         if Self::lex_less_or_equal(b, landmark_u64) {
    //             if first_ub {
    //                 *ub = landmark_u64;
    //                 first_ub = false;
    //             } else {
    //                 *ub = (*ub).min(landmark_u64);
    //             }
    //         }
    //     }
    // }

    // fn convert_widen_bounds_to_wrapped_range(&mut self, lb: u64, ub: u64) {
    //     self.base.lb = lb;
    //     self.base.ub = ub;
    //     self.normalize();
    // }

    // fn make_smaller_interval(a: u64, b: u64, width: u32) -> Self {
    //     let mut res = Self::new_bounds(a, b, width);
    //     res.normalize();
    //     res
    // }
}
