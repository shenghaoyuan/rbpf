/* tnum: tracked (or tristate) numbers for eBPF */
#ifndef __TNUM_H
#define __TNUM_H

#include <stdint.h>
#include <stdbool.h>

typedef unsigned char      u8;
typedef unsigned short     u16;
typedef unsigned int       u32;
typedef unsigned long long u64;

/* tnum结构定义 */
struct tnum {
    u64 value;
    u64 mask;
};

/* 常量定义 */
extern const struct tnum tnum_unknown;
#define TNUM(_v, _m) (struct tnum){.value = _v, .mask = _m}

/* 创建常量tnum */
static inline struct tnum tnum_const(u64 value)
{
    return TNUM(value, 0);
}

/* tnum乘法实现 */
struct tnum tnum_mul(struct tnum a, struct tnum b);

/* 其他tnum相关函数声明 */
struct tnum tnum_add(struct tnum a, struct tnum b);
struct tnum tnum_sub(struct tnum a, struct tnum b);
struct tnum tnum_and(struct tnum a, struct tnum b);
struct tnum tnum_or(struct tnum a, struct tnum b);
struct tnum tnum_xor(struct tnum a, struct tnum b);
struct tnum tnum_lshift(struct tnum a, u8 shift);
struct tnum tnum_rshift(struct tnum a, u8 shift);
struct tnum tnum_range(u64 min, u64 max);

/* 辅助函数 */
bool tnum_in(struct tnum a, struct tnum b);
bool tnum_equals(struct tnum a, struct tnum b);
bool tnum_is_aligned(struct tnum a, u64 size);

#endif /* __TNUM_H */