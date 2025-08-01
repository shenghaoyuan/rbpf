#include <crab/domains/swrapped_interval_impl.hpp>
#include <crab/numbers/bignums.hpp>

namespace ikos {
namespace linear_interval_solver_impl {
using z_swrapped_interval_t = crab::domains::swrapped_interval<z_number>;
using q_swrapped_interval_t = crab::domains::swrapped_interval<q_number>;

template <>
z_swrapped_interval_t mk_interval(z_number c,
				 typename crab::wrapint::bitwidth_t w) {
  return z_swrapped_interval_t::mk_swinterval(c, w);
}
  
template <>
q_swrapped_interval_t mk_interval(q_number c,
				 typename crab::wrapint::bitwidth_t w) {
  return q_swrapped_interval_t::mk_swinterval(c, w);
}

template <>
z_swrapped_interval_t trim_interval(const z_swrapped_interval_t &i,
				   const z_swrapped_interval_t &j) {
  if (i.is_bottom())
    return i;
  // XXX: TODO: gamma(top()) \ gamma(j) can be expressed in a
  //            wrapped interval.
  if (i.is_top())
    return i;
  
  z_swrapped_interval_t trimmed_res = i;
  crab::wrapint::bitwidth_t w = i.get_bitwidth(__LINE__);

  //TODO: both circle single
  if(j.is_singleton()){
    if(j.start_0() ==j.end_0()){ //j-0 circle single
      crab::wrapint k = j.start_0();
      if(i.is_bottom_0() || i.is_top_1()){ // 
        return i;
      }
      bool i_single_0 = (i.start_0() == i.end_0());
      if(i.start_0() == k){
        if(i_single_0){
          trimmed_res.set_start_0(crab::wrapint::get_signed_max(w));
          trimmed_res.set_end_0(crab::wrapint::get_unsigned_min(w));
          trimmed_res.set_is_bottom_0(true);
        }
        crab::wrapint k_plus(k);
        ++k_plus;
        trimmed_res.set_start_0(k_plus);
      }else if(i.end_0() == k){
        if(i_single_0){
          trimmed_res.set_start_0(crab::wrapint::get_signed_max(w));
          trimmed_res.set_end_0(crab::wrapint::get_unsigned_min(w));
          trimmed_res.set_is_bottom_0(true);
        }
        crab::wrapint k_minus(k);
        --k_minus;
        trimmed_res.set_end_0(k_minus);
      }
    }else{//j-1 circle single
      crab::wrapint k = j.start_1();
      if(i.is_bottom_1() || i.is_top_1()){ // 
        return i;
      }
      bool i_single_1 = (i.start_1() == i.end_1());
      if(i.start_1() == k){
        if(i_single_1){
          trimmed_res.set_start_1(crab::wrapint::get_unsigned_max(w));
          trimmed_res.set_end_1(crab::wrapint::get_signed_min(w));
          trimmed_res.set_is_bottom_1(true);
        }
        crab::wrapint k_plus(k);
        ++k_plus;
        trimmed_res.set_start_1(k_plus);
      }else if(i.end_1() == k){
        if(i_single_1){
          trimmed_res.set_start_1(crab::wrapint::get_unsigned_max(w));
          trimmed_res.set_end_1(crab::wrapint::get_signed_min(w));
          trimmed_res.set_is_bottom_1(true);
        }
        crab::wrapint k_minus(k);
        --k_minus;
        trimmed_res.set_end_1(k_minus);
      }
    }
  }
  return trimmed_res;
}

template <>
q_swrapped_interval_t trim_interval(const q_swrapped_interval_t &i,
				   const q_swrapped_interval_t &j) {
  // No refinement possible for disequations over rational numbers
  return i;
}

template <>
z_swrapped_interval_t lower_half_line(const z_swrapped_interval_t &i,
				     bool is_signed) {
  return i.lower_half_line(is_signed);
}

template <>
q_swrapped_interval_t lower_half_line(const q_swrapped_interval_t &i,
				     bool is_signed) {
  return i.lower_half_line(is_signed);
}

template <>
z_swrapped_interval_t lower_half_line(const z_swrapped_interval_t &x,
				     const z_swrapped_interval_t &y, bool is_signed) {
  return y.lower_half_line(x, is_signed);
}

template <>
q_swrapped_interval_t lower_half_line(const q_swrapped_interval_t &x,
				     const q_swrapped_interval_t &y, bool is_signed) {
  return y.lower_half_line(x, is_signed);
}

template <>
z_swrapped_interval_t upper_half_line(const z_swrapped_interval_t &i,
				     bool is_signed) {
  return i.upper_half_line(is_signed);
}
  
template <>
q_swrapped_interval_t upper_half_line(const q_swrapped_interval_t &i,
				     bool is_signed) {
  return i.upper_half_line(is_signed);
}

template <>
z_swrapped_interval_t upper_half_line(const z_swrapped_interval_t &x,
				     const z_swrapped_interval_t &y, bool is_signed) {
  return y.upper_half_line(x, is_signed);
}
  
template <>
q_swrapped_interval_t upper_half_line(const q_swrapped_interval_t &x,
				     const q_swrapped_interval_t &y, bool is_signed) {
  return y.upper_half_line(x, is_signed);
}
} // namespace linear_interval_solver_impl
} // end namespace ikos

namespace crab {
namespace domains {
// Default instantiations
template class swrapped_interval<ikos::z_number>;
} // end namespace domains
} // end namespace crab
