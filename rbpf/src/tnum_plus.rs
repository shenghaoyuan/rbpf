use crate::tnum::Tnum;

/// wrapint
/// 检查值是否为零 (0000...0)
pub fn is_zero(value: u64) -> bool {
    value == 0
}

/// 检查值是否为有符号整数的最大值 (01111...1)
pub fn is_signed_max(value: u64, width: u8) -> bool {
    value == ((1u64 << (width - 1)) - 1)
}

/// 检查值是否为有符号整数的最小值 (1000...0)
pub fn is_signed_min(value: u64, width: u8) -> bool {
    value == (1u64 << (width - 1))
}

/// 检查值是否为无符号整数的最大值 (1111...1)
pub fn is_unsigned_max(value: u64, width: u8) -> bool {
    if width == 64 {
        value == u64::MAX
    } else {
        value == ((1u64 << width) - 1)
    }
}

/// 检查值是否为无符号整数的最小值 (0000...0)
pub fn is_unsigned_min(value: u64) -> bool {
    value == 0
}

/// 获取特定位宽的有符号最大值 (01111...1)
pub fn get_signed_max(width: u8) -> u64 {
    (1u64 << (width - 1)) - 1
}

/// 获取特定位宽的有符号最小值 (1000...0)
pub fn get_signed_min(width: u8) -> u64 {
    1u64 << (width - 1)
}

/// 获取特定位宽的无符号最大值 (1111...1)
pub fn get_unsigned_max(width: u8) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// 获取特定位宽的无符号最小值 (0000...0)
pub fn get_unsigned_min() -> u64 {
    0u64
}

/// 是否为底元素
pub fn tnum_is_bottom(t: &Tnum) -> bool {
    let flag = t.value & t.mask;
    flag != 0
}

/// 是否为顶元素
pub fn tnum_is_top(t: &Tnum, width: u8) -> bool {
    t.value == 0 && t.mask == ((1u64 << width) - 1)
}

/// 是否为负数（最高位为1且确定）
pub fn tnum_is_negative(t: &Tnum, width: u8) -> bool {
    let msb = 1u64 << (width - 1);
    (t.value & msb) != 0 && (t.mask & msb) == 0
}

/// 是否为非负数（最高位为0且确定）
pub fn tnum_is_nonnegative(t: &Tnum, width: u8) -> bool {
    let msb = 1u64 << (width - 1);
    (t.value & msb) == 0 && (t.mask & msb) == 0
}

/// 是否为确定的零
pub fn tnum_is_zero(t: &Tnum) -> bool {
    t.value == 0 && t.mask == 0
}

/// 是否为正数（最高位为0且确定且值不为0）
pub fn tnum_is_positive(t: &Tnum, width: u8) -> bool {
    let msb = 1u64 << (width - 1);
    (t.value & msb) == 0 && (t.mask & msb) == 0 && t.value != 0
}

/// tnum 的有符号除法操作
pub fn tnum_signed_div(a: Tnum, b: Tnum, width: u8) -> Tnum {
    // 处理底值情况
    if tnum_is_bottom(&a) || tnum_is_bottom(&b) {
        return Tnum {
            value: 0,
            mask: 0,
        };
    }

    // 如果两个操作数都是确定值，则进行精确除法
    if a.mask == 0 && b.mask == 0 {
        let a_val = if tnum_is_negative(&a, width) {
            -((a.value as i64).wrapping_neg()) // 确保正确处理有符号数
        } else {
            a.value as i64
        };

        let b_val = if tnum_is_negative(&b, width) {
            -((b.value as i64).wrapping_neg())
        } else {
            b.value as i64
        };

        let result = a_val.wrapping_div(b_val);
        return Tnum::new(result as u64, 0);
    }

    // 如果两个操作数都是非负数，使用无符号除法
    if !tnum_is_negative(&a, width) && !tnum_is_negative(&b, width) {
        return tnum_udiv(&a, &b, width);
    }

    // 创建顶值作为初始结果
    let mut result = Tnum::new(0, get_unsigned_max(width));
    let mut tmp = 0u64;

    // 确定前导位 - 处理不同符号组合
    if tnum_is_negative(&a, width) && tnum_is_negative(&b, width) {
        // 两个负数相除 => 结果为非负数

        // 特殊情况: INT_MIN / -1 会溢出
        if is_signed_min(a.value, width) && is_unsigned_max(b.value, width) && b.mask == 0 {
            return Tnum::new(0, get_unsigned_max(width)); // 返回顶值
        }

        let denom = get_signed_max_value(&b, 64);
        let num = get_signed_min_value(&a, 64);

        // 再次检查可能的溢出
        if !(is_signed_min(num, width) && is_signed_max(denom, width)) {
            // 有符号除法
            let num_i64 = (-(num as i64)).wrapping_neg();
            let denom_i64 = (-(denom as i64)).wrapping_neg();
            tmp = num_i64.wrapping_div(denom_i64) as u64;
        } else {
            tmp = get_signed_max(width);
        }


    } else if tnum_is_negative(&a, width) && tnum_is_positive(&b, width) {
        // 负数除以正数 => 结果为负数
        let neg_max_a = (!get_signed_max_value(&a, 64)).wrapping_add(1);
        if neg_max_a >= get_signed_max_value(&b, 64) {
            let denom = get_signed_min_value(&b, 64);
            let num = get_signed_min_value(&a, 64);
            // 有符号除法
            let num_i64 = (-(num as i64)).wrapping_neg();
            let denom_i64 = denom as i64;
            tmp = num_i64.wrapping_div(denom_i64) as u64;
        }
    } else if tnum_is_positive(&a, width) && tnum_is_negative(&b, width) {
        // 正数除以负数 => 结果为负数
        let neg_min_b = (!get_signed_min_value(&b, 64)).wrapping_add(1);
        if get_signed_min_value(&a, 64) >= neg_min_b {
            let denom = get_signed_max_value(&b, 64);
            let num = get_signed_max_value(&a, 64);
            // 有符号除法
            let num_i64 = num as i64;
            let denom_i64 = (-(denom as i64)).wrapping_neg();
            tmp = num_i64.wrapping_div(denom_i64) as u64;
        }
    }

    // 根据计算结果确定高位模式
    if tmp != 0 {
        if (tmp & (1 << 63)) == 0 {
            // 结果为非负数，确定前导零的数量
            let lead_zero = tmp.leading_zeros() as u64;
            // 清除结果的高位
            result.value &= !(u64::MAX << (64 - lead_zero));
            result.mask &= !(u64::MAX << (64 - lead_zero));
        } else {
            // 结果为负数，确定前导一的数量
            let lead_one = (!tmp).leading_zeros() as u64;
            // 设置结果的高位
            result.value |= u64::MAX << (64 - lead_one);
            result.mask &= !(u64::MAX << (64 - lead_one));
        }
    }

    result
}

/// 表示零圈的 Tnum
pub fn tnum_get_zero_circle(t: &Tnum, width: u8) -> Tnum {
    // 断言：输入不是顶值或底值
    assert!(!tnum_is_top(t, width) && !tnum_is_bottom(t));

    // 计算有符号最大值（符号位为0，其他位为1）
    let sign_max = (1u64 << (width - 1)) - 1;

    // 检查最高位（符号位）
    let msb_mask = 1u64 << (width - 1);

    if (t.value & msb_mask) != 0 {
        // 如果符号位为1，零圈为底值（空集）
        return Tnum {
            value: sign_max,
            mask: sign_max,
        };
    } else if (t.mask & msb_mask) != 0 {
        // 如果符号位不确定，返回符号位确定为0的部分
        return Tnum {
            value: t.value,
            mask: t.mask & sign_max, // 清除掩码的符号位
        };
    } else {
        // 如果符号位确定为0，整个值就是零圈
        return t.clone();
    }
}

/// 获取一圈（负数部分）
pub fn tnum_get_one_circle(t: &Tnum, width: u8) -> Tnum {
    // 断言：输入不是顶值或底值
    assert!(!tnum_is_top(t, width) && !tnum_is_bottom(t));

    // 计算符号位掩码和相关常量
    let msb_mask = 1u64 << (width - 1);

    //这里源代码也没用上sign_max sign_min不知道为什么写
    let sign_max = (1u64 << (width - 1)) - 1;
    let sign_min = 1u64 << (width - 1);

    let unsign_max = u64::MAX >> (64 - width);

    if (t.value & msb_mask) != 0 {
        // 如果符号位为1，整个值就是一圈
        return t.clone();
    } else if (t.mask & msb_mask) != 0 {
        // 如果符号位不确定，返回符号位确定为1的部分
        let mut value = t.value;
        value |= msb_mask; // 设置符号位

        let mut mask = t.mask;
        mask &= !msb_mask; // 清除掩码的符号位

        return Tnum {
            value,
            mask,
        };
    } else {
        // 如果符号位确定为0，一圈为底值（空集）
        return Tnum {
            value: unsign_max,
            mask: unsign_max,
        };
    }
}

/// tnum 的无符号除法操作
pub fn tnum_udiv(a: &Tnum, b: &Tnum, width: u8) -> Tnum {
    // 处理边界情况
    if tnum_is_bottom(a) || tnum_is_bottom(b) {
        return Tnum {
            value: 0,
            mask: 0,
        };
    }

    if tnum_is_top(a, width) || tnum_is_top(b, width) {
        return Tnum::new(0, get_unsigned_max(width));
    }

    // 检查除数是否为零
    let flag = b.value == 0;

    if flag {
        return Tnum::new(0, get_unsigned_max(width));
    } else {
        // 创建顶值作为初始结果
        let mut res = Tnum::new(0, get_unsigned_max(width));

        // 计算最大可能结果
        let max_res = if flag {
            a.value.wrapping_add(a.mask)
        } else {
            // 无符号除法
            a.value.wrapping_add(a.mask).wrapping_div(b.value)
        };

        // 确定前导零的数量
        let lead_z = max_res.leading_zeros() as u64;

        // 清除结果的高位
        res.value &= !(u64::MAX << (64 - lead_z));
        res.mask &= !(u64::MAX << (64 - lead_z));

        // 如果全为零，直接返回
        if lead_z as u8 == width {
            return res;
        }

        ////需要补充
        // 确定低位
        // res = div_compute_low_bit(&res, a, b);

        return res;
    }
}

// pub fn tnum_sdiv(a: &Tnum, b: &Tnum, width: u8) -> Tnum {
//     // 处理边界情况
//     if tnum_is_bottom(a) || tnum_is_bottom(b) {
//         return Tnum { value: 0, mask: 0, is_bottom: true };
//     }

//     if tnum_is_top(a, width) || tnum_is_top(b, width) {
//         return Tnum::new(0, get_unsigned_max(width));
//     }

//     // 检查除数是否为零
//     if b.value == 0 {
//         return Tnum::new(0, get_unsigned_max(width));
//     }
//     // 如果两个操作数都是确定值，执行精确除法
//     else if a.mask == 0 && b.mask == 0 {
//         // 执行有符号除法
//         let a_val = a.value as i64;
//         let b_val = b.value as i64;
//         return Tnum::new(
//             (a_val.wrapping_div(b_val)) as u64,
//             0
//         );
//     }
//     // 处理一般情况：将操作数分解为零圈和一圈，分别计算
//     else {
//         // 获取零圈和一圈
//         let t0 = tnum_get_zero_circle(a, width);
//         let t1 = tnum_get_one_circle(a, width);
//         let x0 = tnum_get_zero_circle(b, width);
//         let x1 = tnum_get_one_circle(b, width);

//         // 如果任何一个圈是底值，跳过对它的计算
//         let mut results = Vec::new();

//         // 计算四种组合的结果
//         if !tnum_is_bottom(&t0) && !tnum_is_bottom(&x0) {
//             results.push(tnum_signed_div(&t0, &x0, width));
//         }

//         if !tnum_is_bottom(&t0) && !tnum_is_bottom(&x1) {
//             results.push(tnum_signed_div(&t0, &x1, width));
//         }

//         if !tnum_is_bottom(&t1) && !tnum_is_bottom(&x0) {
//             results.push(tnum_signed_div(&t1, &x0, width));
//         }

//         if !tnum_is_bottom(&t1) && !tnum_is_bottom(&x1) {
//             results.push(tnum_signed_div(&t1, &x1, width));
//         }

//         // 合并所有结果
//         if results.is_empty() {
//             return Tnum { value: 0, mask: 0, is_bottom: true };
//         }

//         let mut result = results.remove(0);
//         for res in results {
//             result = tnum_or(result, res);
//         }

//         return result;
//     }
// }

/// 表示可能的有符号最大值的 u64
pub fn get_signed_max_value(t: &Tnum, width: u8) -> u64 {
    // 计算最大值（value + mask）
    let max = t.value.wrapping_add(t.mask);

    // 如果掩码的最高位为1（符号位不确定），
    // 则最大值的符号位应该为0（确保为正数）
    let msb_mask = 1u64 << (width - 1);
    if (t.mask & msb_mask) != 0 {
        // 清除最高位，确保结果为正数
        return max & !msb_mask;
    }
    max
}

/// 获取 tnum 可能表示的有符号最小值
pub fn get_signed_min_value(t: &Tnum, width: u8) -> u64 {
    // 最小值起始为原始值
    let mut min = t.value;

    // 如果掩码的最高位为1（符号位不确定），
    // 则最小值的符号位应该为1（确保为负数）
    let msb_mask = 1u64 << (width - 1);
    if (t.mask & msb_mask) != 0 {
        // 设置最高位，确保结果为负数
        min |= msb_mask;
    }
    min
}