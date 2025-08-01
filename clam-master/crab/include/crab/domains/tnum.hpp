#pragma once

#include <crab/domains/interval.hpp>
#include <crab/domains/linear_tnum_solver.hpp>
#include <crab/fixpoint/thresholds.hpp>
#include <crab/numbers/bignums.hpp>
#include <crab/numbers/wrapint.hpp>
#include <crab/support/os.hpp>

/**
 * Machine arithmetic intervals based on the paper
 * "Signedness-Agnostic Program Analysis: Precise Integer Bounds for
 * Low-Level Code" by J.A.Navas, P.Schachte, H.Sondergaard, and
 * P.J.Stuckey published in APLAS'12.
 **/

namespace crab {
namespace domains {

template <typename Number> class tnum {

  wrapint m_value;
  wrapint m_mask;
  bool m_is_bottom;

  using tnum_t = tnum<Number>;
  tnum_t default_implementation(const tnum_t &x) const;
  




  

public:
  using bitwidth_t = wrapint::bitwidth_t;

  tnum();
  tnum(wrapint n);
  tnum(wrapint value, wrapint mask);
  tnum(wrapint value, wrapint mask, bool is_bottom);
  tnum(wrapint min, wrapint max, bitwidth_t width);
  // return top if n does not fit into a wrapint. No runtime errors.
  static tnum_t mk_tnum(Number n, bitwidth_t width);
  // Return top if lb or ub do not fit into a wrapint. No runtime errors.
  static tnum_t mk_tnum(Number lb, Number ub,
                                         bitwidth_t width);
                                     
  tnum_t trim_zero() const;
  static tnum_t tnum_from_range(wrapint min, wrapint max);
  static tnum_t top();
  static tnum_t top(bitwidth_t width);
  static tnum_t bottom();
  static tnum_t bottom(bitwidth_t width);

  // Returns the minimum number of trailing zero bits.
  uint64_t countMinTrailingZeros() const;

  // Returns the maximum number of trailing zero bits possible.
  uint64_t countMaxTrailingZeros() const;

  // Returns the minimum number of leading zero bits.
  uint64_t countMinLeadingZeros() const;

  // Returns the maximum number of leading zero bits possible.
  uint64_t countMaxLeadingZeros() const;

  wrapint getSignedMaxValue() const;

  wrapint getSignedMinValue() const;

  tnum_t getZeroCircle() const;

  tnum_t getOneCircle() const;

  static tnum_t divComputeLowBit(tnum_t Known, const tnum_t &LHS, const tnum_t &RHS);

  tnum_t remGetLowBits(const tnum_t &LHS, const tnum_t &RHS) const;

  bitwidth_t get_bitwidth(int line) const;

  wrapint value() const;

  wrapint mask() const;

  bool is_bottom() const;

  bool is_top() const;

  bool is_negative() const;

  bool is_nonnegative() const;

  bool is_zero() const;

  bool is_positive() const;

  // Interpret wrapint as signed mathematical integers.
  ikos::interval<Number> to_interval() const;
  

  tnum_t lower_half_line(bool is_signed) const;

  tnum_t lower_half_line(const tnum_t &x, bool is_signed) const;

  tnum_t upper_half_line(bool is_signed) const;

  tnum_t upper_half_line(const tnum_t &x, bool is_signed) const;

  bool is_singleton() const;

  // x is a value of tnum.
  bool at(wrapint x) const;

  bool operator<=(const tnum_t &x) const;
  bool operator==(const tnum_t &x) const;
  bool operator!=(const tnum_t &x) const;
  tnum_t operator|(const tnum_t &x) const;
  tnum_t operator&(const tnum_t &x) const;
  tnum_t operator||(const tnum_t &x) const;
  tnum_t widening_thresholds(const tnum_t &x, const thresholds<Number> &ts) const;
  tnum_t operator&&(const tnum_t &x) const;

  tnum_t operator+(const tnum_t &x) const;
  tnum_t &operator+=(const tnum_t &x);
  tnum_t operator-() const;
  tnum_t operator-(const tnum_t &x) const;
  tnum_t &operator-=(const tnum_t &x);
  tnum_t operator*(const tnum_t &x) const;
  tnum_t &operator*=(const tnum_t &x);
  tnum_t operator/(const tnum_t &x) const {
    return SDiv(x);
  }
  tnum_t &operator/=(const tnum_t &x) {
    return this->operator=(this->operator/(x));
  }
  tnum_t signed_div(const tnum_t &x) const;
  tnum_t unsigned_div(const tnum_t &x) const;
  tnum_t SDiv(const tnum_t &x) const;
  tnum_t UDiv(const tnum_t &x) const;
  tnum_t SRem(const tnum_t &x) const;
  tnum_t URem(const tnum_t &x) const;

  tnum_t operator~() const;
  tnum_t ZExt(unsigned bits_to_add) const;
  tnum_t SExt(unsigned bits_to_add) const;
  tnum_t Trunc(unsigned bits_to_keep) const;

  tnum_t Shl(uint64_t k) const;
  tnum_t LShr(uint64_t k) const;
  tnum_t AShr(uint64_t k) const;

  tnum_t Shl(const tnum_t &x) const;
  tnum_t LShr(const tnum_t &x) const;
  tnum_t AShr(const tnum_t &x) const;
  tnum_t And(const tnum_t &x) const;
  tnum_t Or(const tnum_t &x) const;
  tnum_t Xor(const tnum_t &x) const;

  void write(crab::crab_os &o) const;
  friend crab::crab_os &operator<<(crab::crab_os &o,
                                   const tnum<Number> &i) {
    i.write(o);
    return o;
  }
};
} // namespace domains
} // namespace crab

namespace ikos {
namespace linear_interval_solver_impl {
template <>
crab::domains::tnum<ikos::z_number>
mk_interval(ikos::z_number c, typename crab::wrapint::bitwidth_t w);

template <>
crab::domains::tnum<ikos::q_number>
mk_interval(ikos::q_number c, typename crab::wrapint::bitwidth_t w);

template <>
crab::domains::tnum<ikos::z_number>
trim_interval(const crab::domains::tnum<ikos::z_number> &i,
              const crab::domains::tnum<ikos::z_number> &j);

template <>
crab::domains::tnum<ikos::q_number>
trim_interval(const crab::domains::tnum<ikos::q_number> &i,
              const crab::domains::tnum<ikos::q_number> &j);

template <>
crab::domains::tnum<ikos::z_number>
lower_half_line(const crab::domains::tnum<ikos::z_number> &i,
                bool is_signed);

template <>
crab::domains::tnum<ikos::q_number>
lower_half_line(const crab::domains::tnum<ikos::q_number> &i,
                bool is_signed);

// from lb of x, determin the lower_half_line of y
template <>
crab::domains::tnum<ikos::z_number>
lower_half_line(const crab::domains::tnum<ikos::z_number> &x,
                const crab::domains::tnum<ikos::z_number> &y,
                bool is_signed);

template <>
crab::domains::tnum<ikos::q_number>
lower_half_line(const crab::domains::tnum<ikos::q_number> &x,
                const crab::domains::tnum<ikos::q_number> &y,
                bool is_signed);


template <>
crab::domains::tnum<ikos::z_number>
upper_half_line(const crab::domains::tnum<ikos::z_number> &i,
                bool is_signed);

template <>
crab::domains::tnum<ikos::q_number>
upper_half_line(const crab::domains::tnum<ikos::q_number> &i,
                bool is_signed);

template <>
crab::domains::tnum<ikos::z_number>
upper_half_line(const crab::domains::tnum<ikos::z_number> &x,
                const crab::domains::tnum<ikos::z_number> &y, 
                bool is_signed);

template <>
crab::domains::tnum<ikos::q_number>
upper_half_line(const crab::domains::tnum<ikos::q_number> &x,
                const crab::domains::tnum<ikos::q_number> &y,
                bool is_signed);              
} // namespace linear_interval_solver_impl
} // end namespace ikos
