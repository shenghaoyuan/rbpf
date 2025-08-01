#include <crab/domains/stnum_impl.hpp>
#include <crab/numbers/bignums.hpp>

namespace ikos {
namespace linear_interval_solver_impl {
using z_stnum_t = crab::domains::stnum<z_number>;
//using q_stnum_t = crab::domains::stnum<q_number>;

template <>
z_stnum_t mk_interval(z_number c,
				 typename crab::wrapint::bitwidth_t w) {
  return z_stnum_t::mk_stnum(c, w);
}
/*
template <>
q_stnum_t mk_interval(q_number c,
				 typename crab::wrapint::bitwidth_t w) {
  return q_stnum_t::mk_stnum(c, w);
}*/  

template <>
z_stnum_t trim_interval(const z_stnum_t &i,
				   const z_stnum_t &j) {
  return i;
}
/*
template <>
q_stnum_t trim_interval(const q_stnum_t &i,
				   const q_stnum_t &j) {
  // No refinement possible for disequations over rational numbers
  return i;
}*/

template <>
z_stnum_t lower_half_line(const z_stnum_t &i,
				     bool is_signed) {
  return i.lower_half_line(is_signed);
}
/*
template <>
q_stnum_t lower_half_line(const q_stnum_t &i,
				     bool is_signed) {
  return i.lower_half_line(is_signed);
}*/

template <>
z_stnum_t lower_half_line(const z_stnum_t &x, const z_stnum_t &y,
				     bool is_signed) {
  return y.lower_half_line(x, is_signed);
}
/*
template <>
q_stnum_t lower_half_line(const q_stnum_t &x, const q_stnum_t &y,
				     bool is_signed) {
  return y.lower_half_line(x, is_signed);
}*/

template <>
z_stnum_t upper_half_line(const z_stnum_t &i,
				     bool is_signed) {
  return i.upper_half_line(is_signed);
}
/*
template <>
q_stnum_t upper_half_line(const q_stnum_t &i,
				     bool is_signed) {
  return i.upper_half_line(is_signed);
}*/  

template <>
z_stnum_t upper_half_line(const z_stnum_t &x, const z_stnum_t &y,
				     bool is_signed) {
  return y.upper_half_line(x, is_signed);
}
/* 
template <>
q_stnum_t upper_half_line(const q_stnum_t &x, const q_stnum_t &y,
				     bool is_signed) {
  return y.upper_half_line(x, is_signed);
}*/ 

} // namespace linear_interval_solver_impl
} // end namespace ikos

namespace crab {
namespace domains {
// Default instantiations
template class stnum<ikos::z_number>;
} // end namespace domains
} // end namespace crab
