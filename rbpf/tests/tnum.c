// SPDX-License-Identifier: GPL-2.0-only
/* tnum: tracked (or tristate) numbers
 *
 * A tnum tracks knowledge about the bits of a value.  Each bit can be either
 * known (0 or 1), or unknown (x).  Arithmetic operations on tnums will
 * propagate the unknown bits such that the tnum result represents all the
 * possible results for possible values of the operands.
 */
#include <linux/kernel.h>
#include "tnum.h"

#define TNUM(_v, _m)	(struct tnum){.value = _v, .mask = _m}
/* A completely unknown value */
const struct tnum tnum_unknown = { .value = 0, .mask = -1 };

struct tnum tnum_lshift(struct tnum a, u8 shift)
{
	return TNUM(a.value << shift, a.mask << shift);
}

struct tnum tnum_rshift(struct tnum a, u8 shift)
{
	return TNUM(a.value >> shift, a.mask >> shift);
}


struct tnum tnum_add(struct tnum a, struct tnum b)
{
	u64 sm, sv, sigma, chi, mu;

	sm = a.mask + b.mask;
	sv = a.value + b.value;
	sigma = sm + sv;
	chi = sigma ^ sv;
	mu = chi | a.mask | b.mask;
	return TNUM(sv & ~mu, mu);
}

struct tnum tnum_sub(struct tnum a, struct tnum b)
{
	u64 dv, alpha, beta, chi, mu;

	dv = a.value - b.value;
	alpha = dv + a.mask;
	beta = dv - b.mask;
	chi = alpha ^ beta;
	mu = chi | a.mask | b.mask;
	return TNUM(dv & ~mu, mu);
}

struct tnum tnum_and(struct tnum a, struct tnum b)
{
	u64 alpha, beta, v;

	alpha = a.value | a.mask;
	beta = b.value | b.mask;
	v = a.value & b.value;
	return TNUM(v, alpha & beta & ~v);
}



/* Generate partial products by multiplying each bit in the multiplier (tnum a)
 * with the multiplicand (tnum b), and add the partial products after
 * appropriately bit-shifting them. Instead of directly performing tnum addition
 * on the generated partial products, equivalenty, decompose each partial
 * product into two tnums, consisting of the value-sum (acc_v) and the
 * mask-sum (acc_m) and then perform tnum addition on them. The following paper
 * explains the algorithm in more detail: https://arxiv.org/abs/2105.05398.
 */
struct tnum tnum_mul(struct tnum a, struct tnum b)
{
	u64 acc_v = a.value * b.value;
	struct tnum acc_m = TNUM(0, 0);

	while (a.value || a.mask) {
		/* LSB of tnum a is a certain 1 */
		if (a.value & 1)
			acc_m = tnum_add(acc_m, TNUM(0, b.mask));
		/* LSB of tnum a is uncertain */
		else if (a.mask & 1)
			acc_m = tnum_add(acc_m, TNUM(0, b.value | b.mask));
		/* Note: no case for LSB is certain 0 */
		a = tnum_rshift(a, 1);
		b = tnum_lshift(b, 1);
	}
	return tnum_add(TNUM(acc_v, 0), acc_m);
}

bool tnum_in(struct tnum a, struct tnum b)
{
	if (b.mask & ~a.mask)
		return false;
	b.value &= ~a.mask;
	return a.value == b.value;
}