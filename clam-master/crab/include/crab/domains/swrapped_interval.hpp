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

template <typename Number> class swrapped_interval {

  wrapint m_start_0;
  wrapint m_end_0;
  bool m_is_bottom_0;

  wrapint m_start_1;
  wrapint m_end_1;
  bool m_is_bottom_1;


  using swrapped_interval_t = swrapped_interval<Number>;
  swrapped_interval_t default_implementation(const swrapped_interval_t &x) const;
  swrapped_interval(wrapint start0, wrapint end0, bool is_bottom0, 
    wrapint start1, wrapint end1, bool is_bottom1);

/*
  // nsplit in the APLAS'12 paper
  void signed_split(std::vector<swrapped_interval_t> &intervals) const;
  // ssplit in the APLAS'12 paper
  void unsigned_split(std::vector<swrapped_interval_t> &intervals) const;
  // cut in the APLAS'12 paper
  void signed_and_unsigned_split(std::vector<swrapped_interval_t> &out) const;*/

  static swrapped_interval_t split(wrapint start, wrapint end); 

  swrapped_interval_t signed_mul(wrapint start0, wrapint end0, wrapint start1, wrapint end1) const;
  swrapped_interval_t unsigned_mul(wrapint start0, wrapint end0, wrapint start1, wrapint end1) const;
  // if out is empty then the intersection is empty
  /*void exact_meet(const swrapped_interval_t &x,
                  std::vector<swrapped_interval_t> &out) const;*/
  // Perform the reduced product of signed and unsigned multiplication.
  // It uses exact meet rather than abstract meet.
  swrapped_interval_t reduced_signed_unsigned_mul(wrapint start0, wrapint end0, wrapint start1, wrapint end1) const;
  swrapped_interval_t unsigned_div(const swrapped_interval_t &x) const;
  swrapped_interval_t signed_div(const swrapped_interval_t &x) const;
  // This is sound only if wrapped interval defined over z_number.
  swrapped_interval_t trim_zero() const;
  swrapped_interval_t Shl(uint64_t k) const;
  swrapped_interval_t LShr(uint64_t k) const;
  swrapped_interval_t AShr(uint64_t k) const;

public:
  using bitwidth_t = wrapint::bitwidth_t;

  swrapped_interval();
  swrapped_interval(wrapint n);
  swrapped_interval(wrapint start0, wrapint end0, wrapint start1, wrapint end1);

  // return top if n does not fit into a wrapint. No runtime errors.
  static swrapped_interval_t mk_swinterval(Number n, bitwidth_t width);
  // Return top if lb or ub do not fit into a wrapint. No runtime errors.
  static swrapped_interval_t mk_swinterval(Number lb, Number ub,
                                         bitwidth_t width);

  static swrapped_interval_t top();
  static swrapped_interval_t top(bitwidth_t width);
  static swrapped_interval_t bottom();
  static swrapped_interval_t bottom(bitwidth_t width);

/*
  // return interval [0111...1, 1000....0]
  // In the APLAS'12 paper "signed limit" corresponds to "north pole".
  static swrapped_interval_t signed_limit(bitwidth_t b);
  // return interval [1111...1, 0000....0]
  // In the APLAS'12 paper "unsigned limit" corresponds to "south pole".
  static swrapped_interval_t unsigned_limit(bitwidth_t b);

  bool cross_signed_limit() const;
  bool cross_unsigned_limit() const;*/

  bitwidth_t get_bitwidth(int line) const;

  wrapint start_0() const;

  wrapint end_0() const;

  wrapint start_1() const;

  wrapint end_1() const;

  void set_start_0(wrapint n);

  void set_end_0(wrapint n);

  void set_start_1(wrapint n);

  void set_end_1(wrapint n);

  void set_is_bottom_0(bool b);
  
  void set_is_bottom_1(bool b);

  void set_bottom0();

  void set_bottom1();

  void set_circle0(wrapint start, wrapint end, bool flag);

  void set_circle1(wrapint start, wrapint end, bool flag);

  void set_top0();

  void set_top1();

  bool is_bottom_0() const;

  bool is_top_0() const;

  bool is_bottom_1() const;

  bool is_top_1() const;

  bool is_bottom() const;

  bool is_top() const;

  wrapint getSignedMaxValue() const;

  wrapint getSignedMinValue() const;

  wrapint getUnsignedMaxValue() const;

  wrapint getUnsignedMinValue() const;

  // Interpret wrapint as signed mathematical integers.
  ikos::interval<Number> to_interval() const;

  swrapped_interval_t lower_half_line(bool is_signed) const;

  swrapped_interval_t lower_half_line(const swrapped_interval_t &x, bool is_signed) const;

  swrapped_interval_t upper_half_line(bool is_signed) const;

  swrapped_interval_t upper_half_line(const swrapped_interval_t &x, bool is_signed) const;

  bool is_singleton() const;
  bool is_singleton_both_circle() const;

  // Starting from m_start and going clock-wise x is encountered
  // before than m_stop.
  bool at(wrapint x) const;

  bool operator<=(const swrapped_interval_t &x) const;
  bool operator==(const swrapped_interval_t &x) const;
  bool operator!=(const swrapped_interval_t &x) const;
  swrapped_interval_t operator|(const swrapped_interval_t &x) const;
  swrapped_interval_t operator&(const swrapped_interval_t &x) const;
  swrapped_interval_t operator||(const swrapped_interval_t &x) const;
  swrapped_interval_t widening_thresholds(const swrapped_interval_t &x,
                                         const thresholds<Number> &ts) const;
  swrapped_interval_t operator&&(const swrapped_interval_t &x) const;

  swrapped_interval_t operator+(const swrapped_interval_t &x) const;
  swrapped_interval_t &operator+=(const swrapped_interval_t &x);
  swrapped_interval_t operator-() const;
  swrapped_interval_t operator-(const swrapped_interval_t &x) const;
  swrapped_interval_t &operator-=(const swrapped_interval_t &x);
  swrapped_interval_t operator*(const swrapped_interval_t &x) const;
  swrapped_interval_t &operator*=(const swrapped_interval_t &x);
  swrapped_interval_t operator/(const swrapped_interval_t &x) const {
    return SDiv(x);
  }
  swrapped_interval_t &operator/=(const swrapped_interval_t &x) {
    return this->operator=(this->operator/(x));
  }

  swrapped_interval_t SDiv(const swrapped_interval_t &x) const;
  swrapped_interval_t UDiv(const swrapped_interval_t &x) const;
  swrapped_interval_t SRem(const swrapped_interval_t &x) const;
  swrapped_interval_t URem(const swrapped_interval_t &x) const;

  swrapped_interval_t ZExt(unsigned bits_to_add) const;
  swrapped_interval_t SExt(unsigned bits_to_add) const;
  swrapped_interval_t Trunc(unsigned bits_to_keep) const;
  swrapped_interval_t Shl(const swrapped_interval_t &x) const;
  swrapped_interval_t LShr(const swrapped_interval_t &x) const;
  swrapped_interval_t AShr(const swrapped_interval_t &x) const;
  swrapped_interval_t And(const swrapped_interval_t &x) const;
  swrapped_interval_t Or(const swrapped_interval_t &x) const;
  swrapped_interval_t Xor(const swrapped_interval_t &x) const;

  void write(crab::crab_os &o) const;
  friend crab::crab_os &operator<<(crab::crab_os &o,
                                   const swrapped_interval<Number> &i) {
    i.write(o);
    return o;
  }
};
} // namespace domains
} // namespace crab

namespace ikos {
namespace linear_interval_solver_impl {
template <>
crab::domains::swrapped_interval<ikos::z_number>
mk_interval(ikos::z_number c, typename crab::wrapint::bitwidth_t w);

template <>
crab::domains::swrapped_interval<ikos::q_number>
mk_interval(ikos::q_number c, typename crab::wrapint::bitwidth_t w);

template <>
crab::domains::swrapped_interval<ikos::z_number>
trim_interval(const crab::domains::swrapped_interval<ikos::z_number> &i,
              const crab::domains::swrapped_interval<ikos::z_number> &j);

template <>
crab::domains::swrapped_interval<ikos::q_number>
trim_interval(const crab::domains::swrapped_interval<ikos::q_number> &i,
              const crab::domains::swrapped_interval<ikos::q_number> &j);

template <>
crab::domains::swrapped_interval<ikos::z_number>
lower_half_line(const crab::domains::swrapped_interval<ikos::z_number> &i,
                bool is_signed);

template <>
crab::domains::swrapped_interval<ikos::q_number>
lower_half_line(const crab::domains::swrapped_interval<ikos::q_number> &i,
                bool is_signed);

template <>
crab::domains::swrapped_interval<ikos::z_number>
lower_half_line(const crab::domains::swrapped_interval<ikos::z_number> &x,
                const crab::domains::swrapped_interval<ikos::z_number> &y,
                bool is_signed);

template <>
crab::domains::swrapped_interval<ikos::q_number>
lower_half_line(const crab::domains::swrapped_interval<ikos::q_number> &x,
                const crab::domains::swrapped_interval<ikos::q_number> &y,
                bool is_signed);

template <>
crab::domains::swrapped_interval<ikos::z_number>
upper_half_line(const crab::domains::swrapped_interval<ikos::z_number> &i,
                bool is_signed);

template <>
crab::domains::swrapped_interval<ikos::q_number>
upper_half_line(const crab::domains::swrapped_interval<ikos::q_number> &i,
                bool is_signed);

template <>
crab::domains::swrapped_interval<ikos::z_number>
upper_half_line(const crab::domains::swrapped_interval<ikos::z_number> &x,
                const crab::domains::swrapped_interval<ikos::z_number> &y,
                bool is_signed);

template <>
crab::domains::swrapped_interval<ikos::q_number>
upper_half_line(const crab::domains::swrapped_interval<ikos::q_number> &x,
                const crab::domains::swrapped_interval<ikos::q_number> &y,
                bool is_signed);                
} // namespace linear_interval_solver_impl
} // end namespace ikos
