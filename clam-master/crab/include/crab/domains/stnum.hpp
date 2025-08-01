#pragma once

#include <crab/domains/interval.hpp>
#include <crab/domains/tnum.hpp>
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

template <typename Number> class stnum {

  using tnum_t = tnum<Number>;

  tnum_t tnum_0;
  tnum_t tnum_1;
  bool m_is_bottom_0;
  bool m_is_bottom_1;

  using stnum_t = stnum<Number>;
  stnum_t default_implementation(const stnum_t &x) const;
  
  stnum(tnum_t tnum0, tnum_t tnum1, bool is_bottom_0, bool is_bottom_1);



  stnum_t Shl(uint64_t k) const;
  stnum_t LShr(uint64_t k) const;
  stnum_t AShr(uint64_t k) const;

public:
  using bitwidth_t = wrapint::bitwidth_t;

  stnum();
  stnum(wrapint n);

  stnum(tnum_t t0, tnum_t t1);


  stnum(wrapint value_0, wrapint mask_0, wrapint value_1, wrapint mask_1, bool is_bottom_0, bool is_bottom_1);

  stnum(wrapint value_0, wrapint mask_0, wrapint value_1, wrapint mask_1);

  // return top if n does not fit into a wrapint. No runtime errors.
  static stnum_t mk_stnum(Number n, bitwidth_t width);
  // Return top if lb or ub do not fit into a wrapint. No runtime errors.
  static stnum_t mk_stnum(Number lb, Number ub,
                                         bitwidth_t width);
                                     
  static stnum_t normalize(tnum_t a, tnum_t b);

  static stnum_t construct_from_tnum(tnum_t a);

  static stnum_t top();
  static tnum_t top_0();
  static tnum_t top_1();
  static stnum_t top(bitwidth_t width);
  static stnum_t bottom();
  static tnum_t bottom_0();
  static tnum_t bottom_1();
  static stnum_t bottom(bitwidth_t width);

/*
  // Returns the minimum number of trailing zero bits.
  uint64_t countMinTrailingZeros() const;

  // Returns the maximum number of trailing zero bits possible.
  uint64_t countMaxTrailingZeros() const;

  // Returns the minimum number of leading zero bits.
  uint64_t countMinLeadingZeros() const;

  // Returns the maximum number of leading zero bits possible.
  uint64_t countMaxLeadingZeros() const;*/

  wrapint getSignedMaxValue() const;

  wrapint getSignedMinValue() const;

  wrapint getUnsignedMaxValue() const;

  wrapint getUnsignedMinValue() const;

  //static stnum_t divComputeLowBit(stnum_t Known, const stnum_t &LHS, const stnum_t &RHS);

  //stnum_t remGetLowBits(const stnum_t &LHS, const stnum_t &RHS) const;

  bitwidth_t get_bitwidth(int line) const;

  tnum_t get_tnum_0() const;

  tnum_t get_tnum_1() const;

  void set_tnum_0(const tnum_t a);

  void set_tnum_1(const tnum_t a);

  bool is_bottom() const;

  bool is_bottom_0() const;

  bool is_bottom_1() const;

  bool is_top() const;

  bool is_top_0() const;

  bool is_top_1() const;

  bool is_negative() const;

  bool is_nonnegative() const;

  bool is_zero() const;

  bool is_positive() const;

  // Interpret wrapint as signed mathematical integers.
  ikos::interval<Number> to_interval() const;
  
  static tnum_t tnum_from_range_s(wrapint min, wrapint max);

  stnum_t lower_half_line(bool is_signed) const;

  stnum_t lower_half_line(const stnum_t &x, bool is_signed) const;

  stnum_t upper_half_line(bool is_signed) const;

  stnum_t upper_half_line(const stnum_t &x, bool is_signed) const;

  bool is_singleton() const;

  // x is a value of stnum.
  bool at(wrapint x) const;

  bool operator<=(const stnum_t &x) const;
  bool operator==(const stnum_t &x) const;
  bool operator!=(const stnum_t &x) const;
  stnum_t operator|(const stnum_t &x) const;
  stnum_t operator&(const stnum_t &x) const;
  stnum_t operator||(const stnum_t &x) const;
  stnum_t widening_thresholds(const stnum_t &x, const thresholds<Number> &ts) const;
  stnum_t operator&&(const stnum_t &x) const;

  stnum_t operator+(const stnum_t &x) const;
  stnum_t &operator+=(const stnum_t &x);
  stnum_t operator-() const;
  stnum_t operator-(const stnum_t &x) const;
  stnum_t &operator-=(const stnum_t &x);
  stnum_t operator*(const stnum_t &x) const;
  stnum_t &operator*=(const stnum_t &x);
  stnum_t operator/(const stnum_t &x) const {
    return SDiv(x);
  }
  stnum_t &operator/=(const stnum_t &x) {
    return this->operator=(this->operator/(x));
  }

  stnum_t SDiv(const stnum_t &x) const;
  stnum_t UDiv(const stnum_t &x) const;
  stnum_t SRem(const stnum_t &x) const;
  stnum_t URem(const stnum_t &x) const;

  stnum_t operator~() const;
  stnum_t ZExt(unsigned bits_to_add) const;
  stnum_t SExt(unsigned bits_to_add) const;
  stnum_t Trunc(unsigned bits_to_keep) const;
  stnum_t Shl(const stnum_t &x) const;
  stnum_t LShr(const stnum_t &x) const;
  stnum_t AShr(const stnum_t &x) const;
  stnum_t And(const stnum_t &x) const;
  stnum_t Or(const stnum_t &x) const;
  stnum_t Xor(const stnum_t &x) const;

  void write(crab::crab_os &o) const;
  friend crab::crab_os &operator<<(crab::crab_os &o,
                                   const stnum<Number> &i) {
    i.write(o);
    return o;
  }
};
} // namespace domains
} // namespace crab

namespace ikos {
namespace linear_interval_solver_impl {
template <>
crab::domains::stnum<ikos::z_number>
mk_interval(ikos::z_number c, typename crab::wrapint::bitwidth_t w);

template <>
crab::domains::stnum<ikos::q_number>
mk_interval(ikos::q_number c, typename crab::wrapint::bitwidth_t w);

template <>
crab::domains::stnum<ikos::z_number>
trim_interval(const crab::domains::stnum<ikos::z_number> &i,
              const crab::domains::stnum<ikos::z_number> &j);

template <>
crab::domains::stnum<ikos::q_number>
trim_interval(const crab::domains::stnum<ikos::q_number> &i,
              const crab::domains::stnum<ikos::q_number> &j);

template <>
crab::domains::stnum<ikos::z_number>
lower_half_line(const crab::domains::stnum<ikos::z_number> &i,
                bool is_signed);

template <>
crab::domains::stnum<ikos::q_number>
lower_half_line(const crab::domains::stnum<ikos::q_number> &i,
                bool is_signed);

// from lb of x, determin the lower_half_line of y
template <>
crab::domains::stnum<ikos::z_number>
lower_half_line(const crab::domains::stnum<ikos::z_number> &x,
                const crab::domains::stnum<ikos::z_number> &y,
                bool is_signed);

template <>
crab::domains::stnum<ikos::q_number>
lower_half_line(const crab::domains::stnum<ikos::q_number> &x,
                const crab::domains::stnum<ikos::q_number> &y,
                bool is_signed);


template <>
crab::domains::stnum<ikos::z_number>
upper_half_line(const crab::domains::stnum<ikos::z_number> &i,
                bool is_signed);

template <>
crab::domains::stnum<ikos::q_number>
upper_half_line(const crab::domains::stnum<ikos::q_number> &i,
                bool is_signed);

template <>
crab::domains::stnum<ikos::z_number>
upper_half_line(const crab::domains::stnum<ikos::z_number> &x,
                const crab::domains::stnum<ikos::z_number> &y, 
                bool is_signed);

template <>
crab::domains::stnum<ikos::q_number>
upper_half_line(const crab::domains::stnum<ikos::q_number> &x,
                const crab::domains::stnum<ikos::q_number> &y,
                bool is_signed);              
} // namespace linear_interval_solver_impl
} // end namespace ikos
