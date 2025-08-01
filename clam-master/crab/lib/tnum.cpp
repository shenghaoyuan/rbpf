#include <crab/domains/tnum_impl.hpp>
#include <crab/numbers/bignums.hpp>

namespace ikos {
namespace linear_interval_solver_impl {
using z_tnum_t = crab::domains::tnum<z_number>;
using q_tnum_t = crab::domains::tnum<q_number>;

template <>
z_tnum_t mk_interval(z_number c,
				 typename crab::wrapint::bitwidth_t w) {
  return z_tnum_t::mk_tnum(c, w);
}
  
template <>
q_tnum_t mk_interval(q_number c,
				 typename crab::wrapint::bitwidth_t w) {
  return q_tnum_t::mk_tnum(c, w);
}

template <>
z_tnum_t trim_interval(const z_tnum_t &i,
				   const z_tnum_t &j) {
  return i;
}

template <>
q_tnum_t trim_interval(const q_tnum_t &i,
				   const q_tnum_t &j) {
  // No refinement possible for disequations over rational numbers
  return i;
}

template <>
z_tnum_t lower_half_line(const z_tnum_t &i,
				     bool is_signed) {
  return i.lower_half_line(is_signed);
}

template <>
q_tnum_t lower_half_line(const q_tnum_t &i,
				     bool is_signed) {
  return i.lower_half_line(is_signed);
}

template <>
z_tnum_t lower_half_line(const z_tnum_t &x, const z_tnum_t &y,
				     bool is_signed) {
  return y.lower_half_line(x, is_signed);
}

template <>
q_tnum_t lower_half_line(const q_tnum_t &x, const q_tnum_t &y,
				     bool is_signed) {
  return y.lower_half_line(x, is_signed);
}

template <>
z_tnum_t upper_half_line(const z_tnum_t &i,
				     bool is_signed) {
  return i.upper_half_line(is_signed);
}
  
template <>
q_tnum_t upper_half_line(const q_tnum_t &i,
				     bool is_signed) {
  return i.upper_half_line(is_signed);
}

template <>
z_tnum_t upper_half_line(const z_tnum_t &x, const z_tnum_t &y,
				     bool is_signed) {
  return y.upper_half_line(x, is_signed);
}
  
template <>
q_tnum_t upper_half_line(const q_tnum_t &x, const q_tnum_t &y,
				     bool is_signed) {
  return y.upper_half_line(x, is_signed);
}

} // namespace linear_interval_solver_impl
} // end namespace ikos

namespace crab {
namespace domains {
// Default instantiations
template class tnum<ikos::z_number>;
} // end namespace domains
} // end namespace crab
