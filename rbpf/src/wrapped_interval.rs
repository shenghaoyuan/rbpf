//! Wrapped interval implementation for Solana eBPF
//! Based on the paper "A Wrapped Interval Arithmetic" by Jorge A. Navas et al.

use std::cmp::{max, min};

/// 表示一个带位宽的环绕区间 [lb, ub]
#[derive(Debug, Clone)]
pub struct WrappedRange {
    /// 基础范围
    base: BaseRange,
    /// 是否为 bottom
    is_bottom: bool,
    /// widening 计数器
    counter_widening_cannot_doubling: u32,
}

/// 基础范围类型
#[derive(Debug, Clone)]
pub struct BaseRange {
    /// 变量标识符(可以为空)
    var: Option<String>,
    /// 状态改变计数
    num_of_changes: u32,
    /// 是否为格(lattice)
    is_lattice: bool,
    /// 是否为 top
    is_top: bool,
    /// 下界
    lb: u64,
    /// 上界
    ub: u64,
    /// 位宽
    width: u32,
    /// 是否有符号
    is_signed: bool,
}

impl BaseRange {
    fn new_constant(value: u64, width: u32, is_signed: bool) -> Self {
        Self {
            var: None,
            num_of_changes: 0,
            is_lattice: false,
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
            base: BaseRange::new_constant(0, width, false),
            is_bottom: true,
            counter_widening_cannot_doubling: 0,
        }
    }

    /// 创建 top 值
    pub fn top(width: u32) -> Self {
        Self {
            base: BaseRange {
                var: None,
                num_of_changes: 0,
                is_lattice: true,
                is_top: true,
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

    /// 检查是否为 bottom (对应 C++ 的 isBottom)
    pub fn is_bottom(&self) -> bool {
        self.is_bottom
    }

    /// 检查是否为 top (对应 C++ 的 IsTop)
    pub fn is_top(&self) -> bool {
        if self.is_constant() {
            return false;
        }
        self.base.is_top
    }

    /// 检查是否为常量区间 (对应 C++ 的 isConstant)
    fn is_constant(&self) -> bool {
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

    /// 检查最高位是否为1
    fn is_msb_one(&self, x: u64) -> bool {
        x & (1 << (self.base.width - 1)) != 0
    }

    /// 检查最高位是否为0
    fn is_msb_zero(&self, x: u64) -> bool {
        x & (1 << (self.base.width - 1)) == 0
    }

    /// 字典序小于
    fn lex_less_than(&self, x: u64, y: u64) -> bool {
        if self.is_msb_zero(x) && self.is_msb_one(y) {
            false
        } else if self.is_msb_one(x) && self.is_msb_zero(y) {
            true
        } else {
            x < y
        }
    }

    /// 字典序小于等于
    fn lex_less_or_equal(&self, x: u64, y: u64) -> bool {
        self.lex_less_than(x, y) || x == y
    }

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
            max_val - self.base.lb + self.base.ub + 1
        }
    }

    /// 检查是否为零区间
    pub fn is_zero_range(&self) -> bool {
        !self.base.is_top && self.base.lb == 0 && self.base.ub == 0
    }

    /// 检查给定值是否在区间内
    pub fn contains(&self, value: u64) -> bool {
        if self.is_bottom {
            return false;
        }

        if self.base.is_top {
            return true;
        }

        // 处理环绕情况
        if self.base.lb <= self.base.ub {
            value >= self.base.lb && value <= self.base.ub
        } else {
            value >= self.base.lb || value <= self.base.ub
        }
    }

    /// 在北极点分割区间
    pub fn nsplit(x: u64, y: u64, width: u32) -> Vec<Self> {
        // 创建北极点区间 [0111...1, 1000...0]
        let np_lb = (1u64 << (width - 1)) - 1; // 0111...1
        let np_ub = 1u64 << (width - 1); // 1000...0
        let np = Self::new_bounds(np_lb, np_ub, width);

        // 创建临时区间
        let s = Self::new_bounds(x, y, width);

        let mut res = Vec::new();

        // 如果不需要分割
        if !np.wrapped_less_or_equal(&s) {
            res.push(s);
            return res;
        }

        // 分割成两个区间
        // [x, 0111...1]
        res.push(Self::new_bounds(x, np_lb, width));
        // [1000...0, y]
        res.push(Self::new_bounds(np_ub, y, width));

        res
    }

    /// 在南极点分割区间
    pub fn ssplit(x: u64, y: u64, width: u32) -> Vec<Self> {
        // 创建南极点区间 [111...1, 000...0]
        let sp_lb = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        }; // 111...1
        let sp_ub = 0u64; // 000...0
        let sp = Self::new_bounds(sp_lb, sp_ub, width);

        // 创建临时区间
        let s = Self::new_bounds(x, y, width);

        let mut res = Vec::new();

        // 如果不需要分割
        if !sp.wrapped_less_or_equal(&s) {
            res.push(s);
            return res;
        }

        // 分割成两个区间
        // [x, 111...1]
        res.push(Self::new_bounds(x, sp_lb, width));
        // [000...0, y]
        res.push(Self::new_bounds(sp_ub, y, width));

        res
    }

    /// 在南北极点都分割区间
    pub fn psplit(x: u64, y: u64, width: u32) -> Vec<Self> {
        let mut res = Vec::new();

        // 先在北极点分割
        let s1 = Self::nsplit(x, y, width);

        // 对每个分割结果再在南极点分割
        for r in s1.iter() {
            let s2 = Self::ssplit(r.base.lb, r.base.ub, width);
            // 将所有结果添加到结果集
            res.extend(s2);
        }

        res
    }

    /// 移除包含零的区间
    pub fn purge_zero(r: &Self) -> Vec<Self> {
        let mut purged = Vec::new();

        assert!(!(r.base.lb == 0 && r.base.ub == 0), "区间不能为[0,0]");

        let width = r.base.width;
        let zero = Self::new_bounds(0, 0, width);

        if zero.wrapped_less_or_equal(r) {
            if r.base.lb == 0 {
                if r.base.ub != 0 {
                    // 不跨越南极点的情况
                    purged.push(Self::new_bounds(r.base.lb + 1, r.base.ub, width));
                }
            } else {
                if r.base.ub == 0 {
                    // 区间如 [1000,0000]
                    let minus_one = if width >= 64 {
                        u64::MAX
                    } else {
                        (1u64 << width) - 1
                    };
                    purged.push(Self::new_bounds(r.base.lb, minus_one, width));
                } else {
                    // 跨越南极点的情况，分成两个区间
                    let minus_one = if width >= 64 {
                        u64::MAX
                    } else {
                        (1u64 << width) - 1
                    };
                    purged.push(Self::new_bounds(r.base.lb, minus_one, width));
                    purged.push(Self::new_bounds(1, r.base.ub, width));
                }
            }
        } else {
            // 不需要分割
            purged.push(r.clone());
        }

        purged
    }

    /// 批量移除包含零的区间
    pub fn purge_zero_vec(vs: &[Self]) -> Vec<Self> {
        let mut res = Vec::new();
        for v in vs {
            let purged = Self::purge_zero(v);
            res.extend(purged);
        }
        res
    }

    /// 检查是否小于等于
    fn wrapped_less_or_equal(&self, other: &Self) -> bool {
        // 处理 bottom 和 top 的情况
        if self.is_bottom {
            return true;
        }
        if self.is_top() && other.is_top() {
            return true;
        }
        if self.is_top() {
            return false;
        }
        if other.is_top() {
            return true;
        }

        let a = self.base.lb;
        let b = self.base.ub;
        let c = other.base.lb;
        let d = other.base.ub;

        other.contains(a)
            && other.contains(b)
            && (self.is_identical(other) || !self.contains(c) || !self.contains(d))
    }

    /// 检查是否完全相同
    fn is_identical(&self, other: &Self) -> bool {
        self.base.lb == other.base.lb
            && self.base.ub == other.base.ub
            && self.base.width == other.base.width
            && self.is_bottom == other.is_bottom
            && self.base.is_top == other.base.is_top
    }

    /// 检查加减法是否溢出
    fn is_wrapped_overflow_add_sub(a: u64, b: u64, c: u64, d: u64, width: u32) -> bool {
        // 计算两个区间的基数
        let tmp1 = Self::w_card(a, b);
        let tmp2 = Self::w_card(c, d);

        // 如果 tmp1 或 tmp2 不能放入 u64，APInt 会抛出异常
        let n1 = tmp1;
        let n2 = tmp2;
        let max = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width)
        };

        (n1 + n2) > max
    }

    /// 计算环绕基数
    fn w_card(x: u64, y: u64) -> u64 {
        if x <= y {
            y - x + 1
        } else {
            u64::MAX - x + y + 1
        }
    }

    /// 环绕加法运算
    pub fn wrapped_plus(&mut self, op1: &Self, op2: &Self) {
        // [a,b] + [c,d] = [a+c,b+d] if no overflow
        // top           otherwise
        if Self::is_wrapped_overflow_add_sub(
            op1.base.lb,
            op1.base.ub,
            op2.base.lb,
            op2.base.ub,
            self.base.width,
        ) {
            self.make_top();
            return;
        }

        // 执行模运算加法
        self.base.lb = op1.base.lb.wrapping_add(op2.base.lb);
        self.base.ub = op1.base.ub.wrapping_add(op2.base.ub);
        self.normalize();
    }

    /// 环绕减法运算
    pub fn wrapped_minus(&mut self, op1: &Self, op2: &Self) {
        // [a,b] - [c,d] = [a-d,b-c] if no overflow
        // top           otherwise
        if Self::is_wrapped_overflow_add_sub(
            op1.base.lb,
            op1.base.ub,
            op2.base.lb,
            op2.base.ub,
            self.base.width,
        ) {
            self.make_top();
            return;
        }

        // 执行模运算减法
        self.base.lb = op1.base.lb.wrapping_sub(op2.base.ub);
        self.base.ub = op1.base.ub.wrapping_sub(op2.base.lb);
        self.normalize();
    }

    /// 无符号乘法
    fn unsigned_wrapped_mult(op1: &Self, op2: &Self) -> Self {
        let mut res = op1.clone();

        let a = op1.base.lb;
        let b = op1.base.ub;
        let c = op2.base.lb;
        let d = op2.base.ub;

        // 检查溢出
        let (lb, overflow1) = a.overflowing_mul(c);
        let (ub, overflow2) = b.overflowing_mul(d);

        if overflow1 || overflow2 {
            res.make_top();
        } else {
            res.base.lb = lb;
            res.base.ub = ub;
        }

        res
    }

    /// 有符号乘法
    fn signed_wrapped_mult(op1: &Self, op2: &Self) -> Self {
        let mut res = op1.clone();

        let a = op1.base.lb;
        let b = op1.base.ub;
        let c = op2.base.lb;
        let d = op2.base.ub;

        let is_zero_a = !res.is_msb_one(a);
        let is_zero_b = !res.is_msb_one(b);
        let is_zero_c = !res.is_msb_one(c);
        let is_zero_d = !res.is_msb_one(d);

        // [2,5] * [10,20] = [20,100]
        if is_zero_a && is_zero_b && is_zero_c && is_zero_d {
            let (lb, overflow1) = a.overflowing_mul(c);
            let (ub, overflow2) = b.overflowing_mul(d);
            if !overflow1 && !overflow2 {
                res.base.lb = lb;
                res.base.ub = ub;
                return res;
            }
        }
        // [-5,-2] * [-20,-10] = [20,100]
        else if !is_zero_a && !is_zero_b && !is_zero_c && !is_zero_d {
            let (lb, overflow1) = b.overflowing_mul(d);
            let (ub, overflow2) = a.overflowing_mul(c);
            if !overflow1 && !overflow2 {
                res.base.lb = lb;
                res.base.ub = ub;
                return res;
            }
        }
        // [-10,-2] * [2,5] = [-50,-4]
        else if !is_zero_a && !is_zero_b && is_zero_c && is_zero_d {
            let (lb, overflow1) = a.overflowing_mul(d);
            let (ub, overflow2) = b.overflowing_mul(c);
            if !overflow1 && !overflow2 {
                res.base.lb = lb;
                res.base.ub = ub;
                return res;
            }
        }
        // [2,10] * [-5,-2] = [-50,-4]
        else if is_zero_a && is_zero_b && !is_zero_c && !is_zero_d {
            let (lb, overflow1) = b.overflowing_mul(c);
            let (ub, overflow2) = a.overflowing_mul(d);
            if !overflow1 && !overflow2 {
                res.base.lb = lb;
                res.base.ub = ub;
                return res;
            }
        }

        res.make_top();
        res
    }

    /// 环绕乘法运算
    pub fn wrapped_multiplication(&mut self, op1: &Self, op2: &Self) {
        // 处理特殊情况
        if op1.is_zero_range() || op2.is_zero_range() {
            self.base.lb = 0;
            self.base.ub = 0;
            return;
        }

        // 分割区间并计算
        let s1 = Self::psplit(op1.base.lb, op1.base.ub, op1.base.width);
        let s2 = Self::psplit(op2.base.lb, op2.base.ub, op2.base.width);

        self.make_bottom();

        for i1 in s1.iter() {
            for i2 in s2.iter() {
                let tmp1 = Self::unsigned_wrapped_mult(i1, i2);
                let tmp2 = Self::signed_wrapped_mult(i1, i2);
                let mut tmp = Self::new_constant(0, self.base.width);
                tmp.wrapped_meet(&tmp1, &tmp2);
                self.wrapped_join(&tmp);
            }
        }

        self.normalize();
    }

    /// 无符号除法
    fn wrapped_unsigned_division(dividend: &Self, divisor: &Self) -> Self {
        let mut res = dividend.clone();

        let a = dividend.base.lb;
        let b = dividend.base.ub;
        let c = divisor.base.lb;
        let d = divisor.base.ub;

        res.base.lb = a.wrapping_div(d);
        res.base.ub = b.wrapping_div(c);

        res
    }

    /// 有符号除法
    fn wrapped_signed_division(dividend: &Self, divisor: &Self) -> Self {
        let mut res = dividend.clone();

        // 将无符号值转换为有符号值
        let to_signed = |x: u64, width: u32| -> i64 {
            if x & (1 << (width - 1)) != 0 {
                -(((!x + 1) & ((1 << width) - 1)) as i64)
            } else {
                x as i64
            }
        };

        let from_signed = |x: i64, width: u32| -> u64 {
            if x < 0 {
                (!(-x as u64) + 1) & ((1 << width) - 1)
            } else {
                x as u64
            }
        };

        let width = dividend.base.width;
        let a = to_signed(dividend.base.lb, width);
        let b = to_signed(dividend.base.ub, width);
        let c = to_signed(divisor.base.lb, width);
        let d = to_signed(divisor.base.ub, width);

        let div1 = a.checked_div(d).map(|x| from_signed(x, width)).unwrap_or(0);
        let div2 = a.checked_div(c).map(|x| from_signed(x, width)).unwrap_or(0);
        let div3 = b.checked_div(d).map(|x| from_signed(x, width)).unwrap_or(0);
        let div4 = b.checked_div(c).map(|x| from_signed(x, width)).unwrap_or(0);

        res.base.lb = div1.min(div2).min(div3).min(div4);
        res.base.ub = div1.max(div2).max(div3).max(div4);

        res
    }

    /// 环绕除法运算
    pub fn wrapped_division(&mut self, dividend: &Self, divisor: &Self, is_signed: bool) {
        // 处理特殊情况
        if dividend.is_zero_range() {
            self.base.lb = 0;
            self.base.ub = 0;
            return;
        }

        if divisor.is_zero_range() {
            self.make_bottom();
            return;
        }

        if is_signed {
            // 有符号除法
            let s1 = Self::psplit(dividend.base.lb, dividend.base.ub, dividend.base.width);
            let s2 = Self::purge_zero_vec(&Self::psplit(
                divisor.base.lb,
                divisor.base.ub,
                divisor.base.width,
            ));

            self.make_bottom();

            for i1 in s1.iter() {
                for i2 in s2.iter() {
                    let tmp = Self::wrapped_signed_division(i1, i2);
                    self.wrapped_join(&tmp);
                }
            }
        } else {
            // 无符号除法
            let s1 = Self::ssplit(dividend.base.lb, dividend.base.ub, dividend.base.width);
            let s2 = Self::purge_zero_vec(&Self::ssplit(
                divisor.base.lb,
                divisor.base.ub,
                divisor.base.width,
            ));

            self.make_bottom();

            for i1 in s1.iter() {
                for i2 in s2.iter() {
                    let tmp = Self::wrapped_unsigned_division(i1, i2);
                    self.wrapped_join(&tmp);
                }
            }
        }

        self.normalize();
    }

    /// 二元 Join 操作
    pub fn wrapped_join(&mut self, other: &Self) {
        // 处理 bottom 情况
        if other.is_bottom {
            return;
        }
        if self.is_bottom {
            *self = other.clone();
            return;
        }

        // 处理 top 情况
        if other.is_top() || self.is_top() {
            self.make_top();
            return;
        }

        let a = self.base.lb;
        let b = self.base.ub;
        let c = other.base.lb;
        let d = other.base.ub;

        // 包含关系的情况
        if other.wrapped_less_or_equal(self) {
            return;
        }
        if self.wrapped_less_or_equal(other) {
            *self = other.clone();
            return;
        }

        // 一个覆盖另一个的情况
        if other.contains(a) && other.contains(b) && self.contains(c) && self.contains(d) {
            self.make_top();
            return;
        }

        // 重叠的情况
        if self.contains(c) {
            self.base.lb = a;
            self.base.ub = d;
        } else if other.contains(a) {
            self.base.lb = c;
            self.base.ub = b;
        }
        // 左/右倾斜的情况：非确定性情况
        // 这里使用字典序来解决平局
        else if Self::w_card(b, c) == Self::w_card(d, a) {
            if self.lex_less_than(a, c) {
                // 避免跨越北极点
                self.base.lb = a;
                self.base.ub = d;
            } else {
                // 避免跨越北极点
                self.base.lb = c;
                self.base.ub = b;
            }
        } else if self.lex_less_or_equal(Self::w_card(b, c), Self::w_card(d, a)) {
            self.base.lb = a;
            self.base.ub = d;
        } else {
            self.base.lb = c;
            self.base.ub = b;
        }

        self.normalize_top();
        if !self.is_bottom && !other.is_bottom {
            self.reset_bottom_flag();
        }
    }

    /// Meet 操作
    pub fn wrapped_meet(&mut self, v1: &Self, v2: &Self) {
        // 处理 bottom 情况
        if v1.is_bottom || v2.is_bottom {
            self.make_bottom();
            return;
        }

        // 处理 top 情况
        if v1.is_top() {
            *self = v2.clone();
            return;
        }
        if v2.is_top() {
            *self = v1.clone();
            return;
        }

        let a = v1.base.lb;
        let b = v1.base.ub;
        let c = v2.base.lb;
        let d = v2.base.ub;

        // 包含关系的情况
        if v1.wrapped_less_or_equal(v2) {
            *self = v1.clone();
            return;
        }
        if v2.wrapped_less_or_equal(v1) {
            *self = v2.clone();
            return;
        }

        // 一个覆盖另一个的情况
        if v2.contains(a) && v2.contains(b) && v1.contains(c) && v1.contains(d) {
            if self.lex_less_than(Self::w_card(a, b), Self::w_card(c, d))
                || (Self::w_card(a, b) == Self::w_card(c, d) && self.lex_less_or_equal(a, c))
            {
                *self = v1.clone();
            } else {
                *self = v2.clone();
            }
            return;
        }

        // 重叠的情况
        if v1.contains(c) {
            self.base.lb = c;
            self.base.ub = b;
        } else if v2.contains(a) {
            self.base.lb = a;
            self.base.ub = d;
        } else {
            // 不相交的情况
            self.make_bottom();
            return;
        }

        self.normalize_top();
    }

    /// Widening 操作
    pub fn widening(&mut self, previous_v: &Self, jump_set: &[i64]) {
        if previous_v.is_bottom {
            return;
        }

        let old = previous_v.clone();

        let u = old.base.lb;
        let v = old.base.ub;
        let x = self.base.lb;
        let y = self.base.ub;

        let mut can_doubling_interval = true;
        let card_old = Self::w_card(u, v);

        // 溢出检查
        if Self::check_overflow_for_widening_jump(card_old, self.base.width) {
            self.counter_widening_cannot_doubling += 1;
            if self.counter_widening_cannot_doubling < 5 {
                can_doubling_interval = false;
            } else {
                self.make_top();
                self.counter_widening_cannot_doubling = 0;
                return;
            }
        }

        let mut merged = old.clone();
        merged.wrapped_join(self);

        let width = x.count_zeros() as u32;
        if old.wrapped_less_or_equal(self) && !old.contains(x) && !old.contains(y) {
            if !can_doubling_interval {
                let mut widen_lb = x;
                let mut widen_ub = x.wrapping_add(card_old).wrapping_add(card_old);

                let mut jump_lb = 0;
                let mut jump_ub = 0;
                Self::widen_one_interval(
                    merged.base.lb,
                    merged.base.ub,
                    width,
                    jump_set,
                    &mut jump_lb,
                    &mut jump_ub,
                );

                {
                    let tmp = Self::make_smaller_interval(merged.base.lb, widen_lb, width);
                    if tmp.contains(jump_lb) {
                        widen_lb = jump_lb;
                    }
                }
                {
                    let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
                    if tmp.contains(jump_ub) {
                        widen_ub = jump_ub;
                    }
                }

                self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
                let tmp = Self::new_bounds(x, y, width);
                self.wrapped_join(&tmp);
            } else {
                let mut widen_lb = x;
                let mut widen_ub = x.wrapping_add(card_old).wrapping_add(card_old);

                let mut jump_lb = 0;
                let mut jump_ub = 0;
                Self::widen_one_interval(
                    merged.base.lb,
                    merged.base.ub,
                    width,
                    jump_set,
                    &mut jump_lb,
                    &mut jump_ub,
                );

                {
                    let tmp = Self::make_smaller_interval(merged.base.lb, widen_lb, width);
                    if tmp.contains(jump_lb) {
                        widen_lb = jump_lb;
                    }
                }
                {
                    let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
                    if tmp.contains(jump_ub) {
                        widen_ub = jump_ub;
                    }
                }

                self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
                let tmp = Self::new_bounds(x, y, width);
                self.wrapped_join(&tmp);
            }
        } else if merged.base.lb == u && merged.base.ub == y {
            if !can_doubling_interval {
                let mut widen_lb = u;
                let mut widen_ub = u.wrapping_add(card_old).wrapping_add(card_old);

                let mut jump_lb__ = 0;
                let mut jump_ub = 0;
                Self::widen_one_interval(
                    merged.base.lb,
                    merged.base.ub,
                    width,
                    jump_set,
                    &mut jump_lb__,
                    &mut jump_ub,
                );

                {
                    let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
                    if tmp.contains(jump_ub) {
                        widen_ub = jump_ub;
                    }
                }

                self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
                let tmp = Self::new_bounds(u, y, width);
                self.wrapped_join(&tmp);
            } else {
                let mut widen_lb = u;
                let mut widen_ub = u.wrapping_add(card_old).wrapping_add(card_old);

                let mut jump_lb__ = 0;
                let mut jump_ub = 0;
                Self::widen_one_interval(
                    merged.base.lb,
                    merged.base.ub,
                    width,
                    jump_set,
                    &mut jump_lb__,
                    &mut jump_ub,
                );

                {
                    let tmp = Self::make_smaller_interval(merged.base.ub, widen_ub, width);
                    if tmp.contains(jump_ub) {
                        widen_ub = jump_ub;
                    }
                }

                self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
                let tmp = Self::new_bounds(u, y, width);
                self.wrapped_join(&tmp);
            }
        } else if merged.base.lb == x && merged.base.ub == v {
            if !can_doubling_interval {
                let mut widen_lb = 0;
                let mut widen_ub = v;
                let mut widen_ub__ = 0;
                Self::widen_one_interval(
                    merged.base.lb,
                    merged.base.ub,
                    width,
                    jump_set,
                    &mut widen_lb,
                    &mut widen_ub__,
                );
                self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
                let tmp = Self::new_bounds(x, y, width);
                self.wrapped_join(&tmp);
            } else {
                let mut widen_lb = u.wrapping_sub(card_old).wrapping_sub(card_old);
                let mut widen_ub = v;

                let mut jump_lb = 0;
                let mut jump_ub__ = 0;
                Self::widen_one_interval(
                    merged.base.lb,
                    merged.base.ub,
                    width,
                    jump_set,
                    &mut jump_lb,
                    &mut jump_ub__,
                );

                {
                    let tmp = Self::make_smaller_interval(merged.base.lb, widen_lb, width);
                    if tmp.contains(jump_lb) {
                        widen_lb = jump_lb;
                    }
                }

                self.convert_widen_bounds_to_wrapped_range(widen_lb, widen_ub);
                let tmp = Self::new_bounds(x, v, width);
                self.wrapped_join(&tmp);
            }
        } else {
            // 否则，返回旧区间
            self.base.lb = old.base.lb;
            self.base.ub = old.base.ub;
        }

        self.normalize_top();
    }

    /// 泛化的 Join 操作
    pub fn generalized_join(&mut self, values: Vec<&Self>) {
        if values.is_empty() {
            self.make_bottom();
            return;
        }

        // 按照左界的字典序排序
        let mut sorted_values = values.clone();
        sorted_values.sort_by(|a, b| {
            if self.lex_less_or_equal(a.base.lb, b.base.lb) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });

        let mut f = self.clone();
        f.make_bottom();

        // 处理跨越南极点的情况
        for v in sorted_values.iter() {
            if v.is_top() || Self::cross_south_pole(v.base.lb, v.base.ub) {
                f = Self::extend(&f, v);
            }
        }

        let mut g = self.clone();
        g.make_bottom();

        for v in sorted_values.iter() {
            let tmp = Self::clock_wise_gap(&f, v);
            g = Self::bigger(&g, &tmp);
            f = Self::extend(&f, v);
        }

        let tmp = Self::wrapped_complement(&Self::bigger(&g, &Self::wrapped_complement(&f)));
        self.base.lb = tmp.base.lb;
        self.base.ub = tmp.base.ub;
    }

    /// 辅助函数：检查是否跨越南极点
    fn cross_south_pole(x: u64, y: u64) -> bool {
        y < x
    }

    /// 辅助函数：扩展区间
    fn extend(r1: &Self, r2: &Self) -> Self {
        let mut res = r1.clone();
        let mut tmp = r2.clone();
        res.wrapped_join(&tmp);
        res
    }

    /// 辅助函数：选择更大的区间
    fn bigger(r1: &Self, r2: &Self) -> Self {
        if r1.is_bottom() && !r2.is_bottom() {
            return r2.clone();
        }
        if r2.is_bottom() && !r1.is_bottom() {
            return r1.clone();
        }
        if r2.is_bottom() && r1.is_bottom() {
            return r1.clone();
        }

        if Self::lex_less_or_equal_static(
            Self::w_card(r2.base.lb, r2.base.ub),
            Self::w_card(r1.base.lb, r1.base.ub),
        ) {
            r1.clone()
        } else {
            r2.clone()
        }
    }

    /// 辅助函数：计算顺时针间隔
    fn clock_wise_gap(r1: &Self, r2: &Self) -> Self {
        let mut gap = Self::new_bounds(
            r1.base.ub.wrapping_add(1),
            r2.base.lb.wrapping_sub(1),
            r1.base.width,
        );

        if r1.is_bottom() || r2.is_bottom() || r2.contains(r1.base.ub) || r1.contains(r2.base.lb) {
            gap.make_bottom();
        }

        gap
    }

    /// 辅助函数：计算补集
    fn wrapped_complement(r: &Self) -> Self {
        let mut c = r.clone();

        if r.is_bottom() {
            c.make_top();
            return c;
        }
        if r.is_top() {
            c.make_bottom();
            return c;
        }

        c.base.lb = r.base.ub.wrapping_add(1);
        c.base.ub = r.base.lb.wrapping_sub(1);
        c
    }

    /// 逻辑位运算
    pub fn wrapped_logical_bitwise(&mut self, op1: &Self, op2: &Self, op_code: u32) {
        // 重置状态
        self.reset_bottom_flag();
        self.reset_top_flag();

        // 如果任一操作数为 bottom，结果为 bottom
        if op1.is_bottom || op2.is_bottom {
            self.make_bottom();
            return;
        }

        // 如果任一操作数为 top，结果为 top
        if op1.is_top() || op2.is_top() {
            self.make_top();
            return;
        }

        match op_code {
            // AND
            0 => {
                self.base.lb = op1.base.lb & op2.base.lb;
                self.base.ub = op1.base.ub & op2.base.ub;
            }
            // OR
            1 => {
                self.base.lb = op1.base.lb | op2.base.lb;
                self.base.ub = op1.base.ub | op2.base.ub;
            }
            // XOR
            2 => {
                self.base.lb = op1.base.lb ^ op2.base.lb;
                self.base.ub = op1.base.ub ^ op2.base.ub;
            }
            _ => {}
        }

        // 保持位宽不变
        self.base.width = op1.base.width;
        self.normalize();
    }

    /// 位移运算
    pub fn wrapped_bitwise_shifts(&mut self, op1: &Self, op2: &Self, op_code: u32) {
        // 重置状态
        self.reset_bottom_flag();
        self.reset_top_flag();

        // 如果任一操作数为 bottom，结果为 bottom
        if op1.is_bottom || op2.is_bottom {
            self.make_bottom();
            return;
        }

        // 如果任一操作数为 top，结果为 top
        if op1.is_top() || op2.is_top() {
            self.make_top();
            return;
        }

        // 计算位移后保留的位数
        let num_bits_survive_shift = op1.base.width as i64 - op2.base.ub as i64;
        if num_bits_survive_shift <= 0 {
            self.base.lb = 0;
            self.base.ub = 0;
            self.base.width = op1.base.width;
            return;
        }

        match op_code {
            // SHL
            0 => {
                self.base.lb = op1.base.lb << op2.base.lb;
                self.base.ub = op1.base.ub << op2.base.ub;
            }
            // LSHR (逻辑右移)
            1 => {
                let mask = (1 << num_bits_survive_shift) - 1;
                self.base.lb = (op1.base.lb >> op2.base.lb) & mask;
                self.base.ub = (op1.base.ub >> op2.base.ub) & mask;
            }
            // ASHR (算术右移)
            2 => {
                let sign_bit = 1 << (op1.base.width - 1);
                let is_negative = (op1.base.lb & sign_bit) != 0;
                let mask = (1 << num_bits_survive_shift) - 1;

                if is_negative {
                    self.base.lb =
                        ((op1.base.lb >> op2.base.lb) | !mask) & ((1 << op1.base.width) - 1);
                    self.base.ub =
                        ((op1.base.ub >> op2.base.ub) | !mask) & ((1 << op1.base.width) - 1);
                } else {
                    self.base.lb = (op1.base.lb >> op2.base.lb) & mask;
                    self.base.ub = (op1.base.ub >> op2.base.ub) & mask;
                }
            }
            _ => {}
        }

        // 保持位宽不变
        self.base.width = op1.base.width;
        self.normalize();
    }

    /// 取模运算
    pub fn wrapped_rem(&mut self, dividend: &Self, divisor: &Self, is_signed_rem: bool) {
        // 处理特殊情况
        if dividend.is_zero_range() {
            self.base.lb = 0;
            self.base.ub = 0;
            return;
        }
        if divisor.is_zero_range() {
            self.make_bottom();
            return;
        }

        if is_signed_rem {
            let s1 = Self::ssplit(dividend.base.lb, dividend.base.ub, dividend.base.width);
            let s2 = Self::purge_zero_vec(&Self::ssplit(
                divisor.base.lb,
                divisor.base.ub,
                divisor.base.width,
            ));

            // 确保除数不为空（不应该包含区间[0,0]）
            assert!(!s2.is_empty(), "Sanity check: empty means interval [0,0]");

            self.make_bottom();

            for i1 in s1.iter() {
                for i2 in s2.iter() {
                    let a = i1.base.lb;
                    let b = i1.base.ub;
                    let c = i2.base.lb;
                    let d = i2.base.ub;

                    let is_zero_a = !self.is_msb_one(a);
                    let is_zero_c = !self.is_msb_one(c);

                    let width = self.base.width;
                    let (lb, ub) = if is_zero_a && is_zero_c {
                        // [0,d-1]
                        (0, d.wrapping_sub(1))
                    } else if is_zero_a && !is_zero_c {
                        // [0,-c-1]
                        (0, c.wrapping_neg().wrapping_sub(1))
                    } else if !is_zero_a && is_zero_c {
                        // [-d+1,0]
                        (d.wrapping_sub(1).wrapping_neg(), 0)
                    } else if !is_zero_a && !is_zero_c {
                        // [c+1,0]
                        (c.wrapping_add(1), 0)
                    } else {
                        unreachable!("This should be unreachable!");
                    };

                    let mut tmp = Self::new_bounds(lb, ub, width);
                    self.wrapped_join(&tmp);
                }
            }
        } else {
            // 无符号取模：在南极点分割并计算每个笛卡尔积元素的无符号操作
            let s1 = Self::ssplit(dividend.base.lb, dividend.base.ub, dividend.base.width);
            let s2 = Self::purge_zero_vec(&Self::ssplit(
                divisor.base.lb,
                divisor.base.ub,
                divisor.base.width,
            ));

            // 确保除数不为空（不应该包含区间[0,0]）
            assert!(!s2.is_empty(), "Sanity check: empty means interval [0,0]");

            self.make_bottom();

            for i1 in s1.iter() {
                for i2 in s2.iter() {
                    // 这是一个可以改进精度的特殊情况
                    // 也可以用于有符号情况
                    // 这在期刊版本中有描述
                    // let div = Self::wrapped_unsigned_division(i1, i2);
                    // if Self::w_card(div.base.lb, div.base.ub) == 1 {
                    //     let mut tmp1 = i2.clone();
                    //     let mut tmp2 = i2.clone();
                    //     tmp1.wrapped_minus(i1, &div);
                    //     tmp2.wrapped_multiplication(&tmp1, i2);
                    //     self.wrapped_join(&tmp2);
                    // } else {
                    let d = i2.base.ub;
                    let lb = 0;
                    let ub = d.wrapping_sub(1);
                    let mut tmp = Self::new_bounds(lb, ub, self.base.width);
                    self.wrapped_join(&tmp);
                    // }
                }
            }
        }

        self.normalize();
    }

    /// 同半球有符号小于等于比较
    fn comparison_sle_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
        i1.base.lb <= i2.base.ub
    }

    /// 同半球有符号小于比较
    fn comparison_slt_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
        i1.base.lb < i2.base.ub
    }

    /// 同半球无符号小于等于比较
    fn comparison_ule_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
        i1.base.lb <= i2.base.ub
    }

    /// 同半球无符号小于比较
    fn comparison_ult_same_hemisphere(&self, i1: &Self, i2: &Self) -> bool {
        i1.base.lb < i2.base.ub
    }

    /// 有符号小于比较
    fn comparison_signed_less_than(&self, i1: &Self, i2: &Self, is_strict: bool) -> bool {
        // 在北极点分割并对所有可能的对进行正常测试
        // 如果有一个为真则返回真
        let s1 = Self::nsplit(i1.base.lb, i1.base.ub, i1.base.width);
        let s2 = Self::nsplit(i2.base.lb, i2.base.ub, i2.base.width);

        let mut tmp = false;
        for i1 in s1.iter() {
            for i2 in s2.iter() {
                if is_strict {
                    tmp |= self.comparison_slt_same_hemisphere(i1, i2);
                } else {
                    tmp |= self.comparison_sle_same_hemisphere(i1, i2);
                }
                if tmp {
                    return true;
                }
            }
        }
        tmp
    }

    /// 无符号小于比较
    fn comparison_unsigned_less_than(&self, i1: &Self, i2: &Self, is_strict: bool) -> bool {
        let s1 = Self::ssplit(i1.base.lb, i1.base.ub, i1.base.width);
        let s2 = Self::ssplit(i2.base.lb, i2.base.ub, i2.base.width);

        let mut tmp = false;
        for i1 in s1.iter() {
            for i2 in s2.iter() {
                if is_strict {
                    tmp |= self.comparison_ult_same_hemisphere(i1, i2);
                } else {
                    tmp |= self.comparison_ule_same_hemisphere(i1, i2);
                }
                if tmp {
                    return true;
                }
            }
        }
        tmp
    }

    /// 有符号小于等于
    pub fn comparison_sle(&self, other: &Self) -> bool {
        self.comparison_signed_less_than(self, other, false)
    }

    /// 有符号小于
    pub fn comparison_slt(&self, other: &Self) -> bool {
        self.comparison_signed_less_than(self, other, true)
    }

    /// 无符号小于等于
    pub fn comparison_ule(&self, other: &Self) -> bool {
        self.comparison_unsigned_less_than(self, other, false)
    }

    /// 无符号小于
    pub fn comparison_ult(&self, other: &Self) -> bool {
        self.comparison_unsigned_less_than(self, other, true)
    }

    /// 规范化区间
    fn normalize(&mut self) {
        // 如果是 bottom 或 top，不需要规范化
        if self.is_bottom || self.base.is_top {
            return;
        }

        // 检查是否需要设置为 top
        let max_val = if self.base.width >= 64 {
            u64::MAX
        } else {
            (1u64 << self.base.width) - 1
        };

        if self.base.lb > max_val || self.base.ub > max_val {
            self.make_top();
            return;
        }

        // 规范化上下界
        self.base.lb &= max_val;
        self.base.ub &= max_val;
    }

    /// 规范化为 top (对应 C++ 的 normalizeTop)
    fn normalize_top(&mut self) {
        // 如果是 bottom，不需要规范化
        if self.is_bottom {
            return;
        }

        // 检查是否需要设置为 top
        let max_val = if self.base.width >= 64 {
            u64::MAX
        } else {
            (1u64 << self.base.width) - 1
        };

        if self.base.lb == 0 && self.base.ub == max_val {
            self.make_top();
        }
    }

    fn check_overflow_for_widening_jump(card: u64, width: u32) -> bool {
        // 计算最大值 2^(w-1)
        let max = if width <= 1 { 0 } else { 1u64 << (width - 1) };
        card >= max
    }

    fn widen_one_interval(
        a: u64,
        b: u64,
        width: u32,
        jump_set: &[i64],
        lb: &mut u64,
        ub: &mut u64,
    ) {
        // 初始化为最小值和最大值
        *lb = if width >= 64 { u64::MIN } else { 0 };
        *ub = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };

        let mut first_lb = true;
        let mut first_ub = true;

        for &landmark in jump_set {
            let landmark_u64 = landmark as u64;
            if Self::lex_less_or_equal_static(landmark_u64, a) {
                if first_lb {
                    *lb = landmark_u64;
                    first_lb = false;
        } else {
                    *lb = (*lb).max(landmark_u64);
                }
            }
            if Self::lex_less_or_equal_static(b, landmark_u64) {
                if first_ub {
                    *ub = landmark_u64;
                    first_ub = false;
            } else {
                    *ub = (*ub).min(landmark_u64);
                }
            }
        }
    }

    fn convert_widen_bounds_to_wrapped_range(&mut self, lb: u64, ub: u64) {
        self.base.lb = lb;
        self.base.ub = ub;
        self.normalize();
    }

    fn make_smaller_interval(a: u64, b: u64, width: u32) -> Self {
        let mut res = Self::new_bounds(a, b, width);
        res.normalize();
        res
    }

    fn lex_less_or_equal_static(x: u64, y: u64) -> bool {
        x <= y
    }
}
