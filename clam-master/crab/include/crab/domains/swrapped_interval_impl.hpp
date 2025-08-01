#pragma once

#include <crab/domains/swrapped_interval.hpp>
#include <crab/support/debug.hpp>
#include <crab/support/stats.hpp>

#include <boost/optional.hpp>

#define PRINT_WRAPINT_AS_SIGNED

namespace crab {
namespace domains {

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::default_implementation(
    const swrapped_interval<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  } else {
    return swrapped_interval<Number>::top();
  }
}

template <typename Number>
swrapped_interval<Number>::swrapped_interval(wrapint start0, wrapint end0, bool is_bottom0, 
    wrapint start1, wrapint end1, bool is_bottom1)
    : m_start_0(start0), m_end_0(end0), m_is_bottom_0(is_bottom0),
      m_start_1(start1), m_end_1(end1), m_is_bottom_1(is_bottom1) {
        //crab::outs() << "swrapped_interval1" << "\n";
  if (start0.get_bitwidth() != end0.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in zero sign interval");
  }
  if (start1.get_bitwidth() != end1.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in one sign interval");
  }
  if (start0.get_bitwidth() != end1.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in sign interval");
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::split(wrapint start, wrapint end){
  //crab::outs() << "split&" << "\n";
  bool sm = start.msb();
  bool em = end.msb();
  wrapint::bitwidth_t b = start.get_bitwidth();
  if(!sm && !em){
    if(start <= end){
      return swrapped_interval<Number>(start, end, false, 
        wrapint::get_unsigned_max(b), wrapint::get_signed_min(b), true);
    }else{
      return swrapped_interval<Number>::top(b);
    }
  }else if(!sm && em){
    return swrapped_interval<Number>(start, wrapint::get_signed_max(b), false,
      wrapint::get_signed_min(b), end, false);
  }else if(sm && !em){
    return swrapped_interval<Number>(wrapint::get_unsigned_min(b), end, false,
      start, wrapint::get_unsigned_max(b), false);
  }else{
    if(start <= end){
      return swrapped_interval<Number>(wrapint::get_signed_max(b), wrapint::get_unsigned_min(b), true, 
        start, end, false);
    }else{
      return swrapped_interval<Number>::top(b);
    }
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::signed_mul(wrapint start0, wrapint end0, 
  wrapint start1, wrapint end1) const {
  bool msb0 = start0.msb();
  bool msb1 = start1.msb();
  wrapint::bitwidth_t b = start0.get_bitwidth();
  swrapped_interval<Number> res = swrapped_interval<Number>::top(b);
  
  
  if(msb0 == msb1){
    if(!msb0){
      return unsigned_mul(start0, end0, start1, end1);
    }else{
      if ((start0.get_unsigned_bignum() * start1.get_unsigned_bignum()) -
              (end0.get_unsigned_bignum() * end1.get_unsigned_bignum()) <
          wrapint::get_unsigned_max(b).get_unsigned_bignum()) {
        res = split(end0 * end1, start0 * start1);
      }
    }
  }else if(!msb0 && msb1){
    if ((start0.get_unsigned_bignum() * end1.get_unsigned_bignum()) -
              (end0.get_unsigned_bignum() * start1.get_unsigned_bignum()) <
          wrapint::get_unsigned_max(b).get_unsigned_bignum()) {
        res = split(end0 * start1, start0 * end1);
      }
  }else{
    if ((end0.get_unsigned_bignum() * start1.get_unsigned_bignum()) -
              (start0.get_unsigned_bignum() * end1.get_unsigned_bignum()) <
          wrapint::get_unsigned_max(b).get_unsigned_bignum()) {
        res = split(start0 * end1, end0 * start1);
      }
  }
  //CRAB_LOG("swrapped-imply-mul", crab::outs() << "("<< start0 << ", " << end0  << ") *_s (" << start1 << ", " << end1  << ") = " << res << "\n";);

  return res;
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::unsigned_mul(wrapint start0, wrapint end0, 
  wrapint start1, wrapint end1) const {

  wrapint res_start = start0 * start1;
  wrapint res_end = end0 * end1;

  wrapint::bitwidth_t b = start0.get_bitwidth();
  
  if ((end0.get_unsigned_bignum() * end1.get_unsigned_bignum()) -
      (start0.get_unsigned_bignum() * start1.get_unsigned_bignum()) <
      wrapint::get_unsigned_max(b).get_unsigned_bignum()) {
    return split(res_start, res_end);
  }else{
    return swrapped_interval<Number>::top(b);
  }
}



// Perform the reduced product of signed and unsigned multiplication.
// It uses exact meet rather than abstract meet.
template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::reduced_signed_unsigned_mul(
    wrapint start0, wrapint end0, wrapint start1, wrapint end1) const {

  swrapped_interval<Number> s = signed_mul(start0, end0, start1, end1);
  swrapped_interval<Number> u = unsigned_mul(start0, end0, start1, end1);
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "("<< start0 << ", " << end0  << ") *_u (" << start1 << ", " << end1  << ") = " << u << "\n";);
  return s & u;
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::unsigned_div(
    const swrapped_interval<Number> &x) const {
  
  assert(!x.at(wrapint(0, x.get_bitwidth(__LINE__))));
  assert(!is_bottom() && !x.is_bottom());


  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  swrapped_interval_t res00 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res01 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res10 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res11 = swrapped_interval<Number>::bottom(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  wrapint zero(0, b);
  wrapint one(1, b);

  bool t0_bottom = is_bottom_0();
  bool t1_bottom = is_bottom_1();
  bool x0_bottom = x.is_bottom_0();
  bool x1_bottom = x.is_bottom_1();

  if(!t0_bottom && !x0_bottom){
    res00 = swrapped_interval<Number>(m_start_0.udiv(x.m_end_0), m_end_0.udiv(x.m_start_0), false,
      unsign_max, sign_min, true);
  }
  if(!t0_bottom && !x1_bottom){
    res01 = swrapped_interval<Number>(zero, zero, false, unsign_max, sign_min, true);
  }
  if(!t1_bottom && !x0_bottom){
    if(x.m_start_0 == one){
      res10 = swrapped_interval<Number>(m_start_1.udiv(x.m_end_0), m_end_1.udiv(x.m_start_0), false,
        m_start_1, m_end_1, false);
    }else{
      res10 = swrapped_interval<Number>(m_start_1.udiv(x.m_end_0), m_end_1.udiv(x.m_start_0), false,
        unsign_max, sign_min, true);
    }
  }
  if(!t1_bottom && !x1_bottom){
    res11 = swrapped_interval<Number>(m_start_1.udiv(x.m_end_1), m_end_1.udiv(x.m_start_1), false,
      unsign_max, sign_min, true);
  }
swrapped_interval<Number> res = res00 | res01 | res10 | res11;
  //CRAB_LOG("swrapped-imply-unsign-div", crab::outs() << *this << " /_u " << x << "=" << res << "\n";);
  return res;
}



template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::signed_div(const swrapped_interval<Number> &x) const {
 //CRAB_LOG("swrapped-imply", crab::outs() << *this << " /_u " << x << "=";);
  assert(!x.at(wrapint(0, x.get_bitwidth(__LINE__))));
  assert(!is_bottom() && !x.is_bottom());


  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  swrapped_interval_t res00 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res01 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res10 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res11 = swrapped_interval<Number>::bottom(b);
  wrapint sign_max = wrapint::get_signed_max(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  wrapint unsign_min = wrapint::get_unsigned_min(b);
  wrapint minus_one(-1, b);

  bool t0_bottom = is_bottom_0();
  bool t1_bottom = is_bottom_1();
  bool x0_bottom = x.is_bottom_0();
  bool x1_bottom = x.is_bottom_1();

  if(!t0_bottom && !x0_bottom){
    res00 = swrapped_interval<Number>(m_start_0.sdiv(x.m_end_0), m_end_0.sdiv(x.m_start_0), false,
      unsign_max, sign_min, true);
  }
  if(!t0_bottom && !x1_bottom){
    //res01 = swrapped_interval<Number>(sign_max, unsign_min, true, 
    //  m_end_0.sdiv(x.m_end_1), m_start_0.sdiv(x.m_start_1), false);
    res01 = split(m_end_0.sdiv(x.m_end_1), m_start_0.sdiv(x.m_start_1));// 
  }
  if(!t1_bottom && !x0_bottom){
    //res10 = swrapped_interval<Number>(sign_max, unsign_min, true,
    //  m_start_1.sdiv(x.m_start_0), m_end_1.sdiv(x.m_end_0), false);
    wrapint tmp_start = m_start_1.sdiv(x.m_start_0);
    wrapint tmp_end = m_end_1.sdiv(x.m_end_0);
    res10 = split(tmp_start, tmp_end); // 
  }
  if(!t1_bottom && !x1_bottom){
    if((m_end_1 == sign_min && x.m_start_1 == minus_one) 
          || (m_start_1 == sign_min && x.m_end_1 == minus_one)){// overflow case
      return swrapped_interval<Number>::top();
    }else{// no overflow case
      res11 = swrapped_interval<Number>(m_end_1.sdiv(x.m_start_1), m_start_1.sdiv(x.m_end_1), false,
        unsign_max, sign_min, true);
    }
  }
  swrapped_interval<Number> res = res00 | res01 | res10 | res11;
//CRAB_LOG("swrapped-imply-signed_div", crab::outs() << *this << " /_u " << x << "=" << res << "\n";);
//CRAB_LOG("swrapped-imply-signed_div", crab::outs() << " /_u res11= " << res11 <<"\n";);
  return res00 | res01 | res10 | res11;
}

/// This is sound only if wrapped interval defined over z_number.
template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::trim_zero() const {
  swrapped_interval<Number> res(m_start_0, m_end_0, m_is_bottom_0,
                             m_start_1, m_end_1, m_is_bottom_1);
  wrapint zero(0, get_bitwidth(__LINE__));
  if (!is_bottom() && (!(*this == zero))) {
    if (start_0() == zero) {
      //this->m_start_0 = wrapint(1, get_bitwidth(__LINE__));
      return swrapped_interval<Number>(wrapint(1, get_bitwidth(__LINE__)), m_end_0, m_is_bottom_0,
                             m_start_1, m_end_1, m_is_bottom_1);
    } 
  }
  return res;
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::Shl(uint64_t k) const {
   //CRAB_LOG("swrapped-imply", crab::outs() << *this <<  "shl " << k << "\n";);
  if (is_bottom())
    return *this;
  if(is_top()) // need to fix
    return *this;

  // XXX: we need the check is_top before calling get_bitwidth();

  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  wrapint up(1, width);
  up = up << wrapint(k, width);
  up = up - wrapint(1, width);
  up = ~up;

  // TODO: return (0, 1..10..0) where #1's = b-k and #0's= k
  return swrapped_interval<Number>(wrapint::get_unsigned_min(width), wrapint::get_signed_max(width), false,
          wrapint::get_signed_min(width), up, false);
  
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::LShr(uint64_t k) const {
 // CRAB_LOG("swrapped-imply", crab::outs() << *this <<  "lshr " << k << "\n";);
  if (is_bottom())
    return *this;
  if(is_top()) // need to fix
    return *this;

  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  bool bottom0 = is_bottom_0();
  bool bottom1 = is_bottom_1();
  if(!bottom0 && !bottom1){
    return swrapped_interval<Number>(m_start_0.lshr(wrapint(k, width)), m_end_1.lshr(wrapint(k, width)),  false,
          wrapint::get_unsigned_max(width), wrapint::get_signed_min(width), true);
  }else if(!bottom0 && bottom1){
    return swrapped_interval<Number>(m_start_0.lshr(wrapint(k, width)), m_end_0.lshr(wrapint(k, width)),  false,
          wrapint::get_unsigned_max(width), wrapint::get_signed_min(width), true);
  }else if(bottom0 && !bottom1){
    wrapint startl = m_start_1.lshr(wrapint(k, width)); //  , need to improve
    if(startl.msb()) return top(width); 


    return swrapped_interval<Number>(m_start_1.lshr(wrapint(k, width)), m_end_1.lshr(wrapint(k, width)),  false,
          wrapint::get_unsigned_max(width), wrapint::get_signed_min(width), true);
  }
  
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::AShr(uint64_t k) const {
  //CRAB_LOG("swrapped-imply", crab::outs() << *this <<  "ashr " << k << "\n";);
  if (is_bottom())
    return *this;
  if(is_top()) // need to fix
    return *this;

  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  wrapint kw(k, width);
  bool bottom0 = is_bottom_0();
  bool bottom1 = is_bottom_1();
  if(!bottom0 && !bottom1){
    return swrapped_interval<Number>(m_start_0.ashr(kw), m_end_0.ashr(kw),  false,
          m_start_1.ashr(kw), m_end_1.ashr(kw), false);
  }else if(!bottom0 && bottom1){
    return swrapped_interval<Number>(m_start_0.ashr(kw), m_end_0.ashr(kw),  false,
          wrapint::get_signed_min(width), wrapint::get_unsigned_max(width), true);
  }else if(bottom0 && !bottom1){
    return swrapped_interval<Number>(wrapint::get_signed_max(width), wrapint::get_unsigned_min(width),  true,
          m_start_1.ashr(kw), m_end_1.ashr(kw), false);
  }
  
}

template <typename Number>
swrapped_interval<Number>::swrapped_interval(wrapint n) : m_start_0(n), m_end_0(n), m_is_bottom_0(false),
    m_start_1(n), m_end_1(n), m_is_bottom_1(false) {
      //crab::outs() << "swrapped_interval2" << "\n";
  wrapint::bitwidth_t w = n.get_bitwidth();
  if(w==1){
    m_start_0 = wrapint(0, 1);
    m_end_0 =  wrapint(0, 1);
    m_start_1 = wrapint(1, 1);
    m_end_1 = wrapint(1, 1);
    if(n.is_zero()){
      m_is_bottom_0 = false;
      m_is_bottom_1 = true;
    }else{
      m_is_bottom_0 = true;
      m_is_bottom_1 = false;
    }
  }else{
    if(n.msb()){
      m_start_0 = wrapint::get_signed_max(w);
      m_end_0 =  wrapint::get_unsigned_min(w);
      m_is_bottom_0 = true;
      m_start_1 = n;
      m_end_1 = n;
      m_is_bottom_1 = false;
    }else{
      m_start_0 = n;
      m_end_0 =  n;
      m_is_bottom_0 = false;
      m_start_1 = wrapint::get_unsigned_max(w);
      m_end_1 = wrapint::get_signed_min(w);
      m_is_bottom_1 = true;
    }
  }
}
/*
template <typename Number>
swrapped_interval<Number>::swrapped_interval(wrapint n) : m_start_0(n), m_end_0(n), m_is_bottom_0(false),
    m_start_1(n), m_end_1(n), m_is_bottom_1(false) {
      crab::outs() << "swrapped_interval2" << "\n";
  wrapint::bitwidth_t w = n.get_bitwidth();
  crab::outs() << "swrapped_interval2.0" << "\n";
  if(w==1){
    crab::outs() << "swrapped_interval2.1" << "\n";
    this->m_start_0 = wrapint(0, 1);
    this->m_end_0 =  wrapint(0, 1);
    this->m_start_1 = wrapint(1, 1);
    this->m_end_1 = wrapint(1, 1);
    if(n.is_zero()){
      this->m_is_bottom_0 = false;
      this->m_is_bottom_1 = true;
    }else{
      this->m_is_bottom_0 = true;
      this->m_is_bottom_1 = false;
    }
  }else{
    crab::outs() << "swrapped_interval2.2" << "\n";
    if(n.msb()){
      crab::outs() << "swrapped_interval2.3" << "\n";
      this->m_start_0 = wrapint::get_signed_max(w);
      crab::outs() << "swrapped_interval2.4" << "\n";
      this->m_end_0 =  wrapint::get_unsigned_min(w);
      crab::outs() << "swrapped_interval2.5" << "\n";
      this->m_is_bottom_0 = true;
      crab::outs() << "swrapped_interval2.6" << "\n";
      this->m_start_1 = n;
      crab::outs() << "swrapped_interval2.7" << "\n";
      this->m_end_1 = n;
      crab::outs() << "swrapped_interval2.8" << "\n";
      this->m_is_bottom_1 = false;
    }else{
      this->m_start_0 = n;
      this->m_end_0 =  n;
      this->m_is_bottom_0 = false;
      this->m_start_1 = wrapint::get_unsigned_max(w);
      this->m_end_1 = wrapint::get_signed_min(w);
      this->m_is_bottom_1 = true;
    }
    crab::outs() << "swrapped_interval2.9" << "\n";
  }
}
*/

template <typename Number>
swrapped_interval<Number>::swrapped_interval(wrapint start0, wrapint end0, 
  wrapint start1, wrapint end1)
    : m_start_0(start0), m_end_0(end0), m_is_bottom_0(false),
      m_start_1(start1), m_end_1(end1), m_is_bottom_1(false) {
        //crab::outs() << "swrapped_interval3" << "\n";
  if (start0.get_bitwidth() != end0.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in zero sign interval");
  }
  if (start1.get_bitwidth() != end1.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in one sign interval");
  }
  if (start0.get_bitwidth() != end1.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in sign interval");
  }
}

// To represent top, the particular bitwidth here is irrelevant. We
// just make sure that m_end - m_start == get_max()
template <typename Number>
swrapped_interval<Number>::swrapped_interval()
    : m_start_0(wrapint(0, 3)), m_end_0(3, 3), m_is_bottom_0(false),
      m_start_1(wrapint(4, 3)), m_end_1(7, 3), m_is_bottom_1(false) {}

// return top if n does not fit into a wrapint. No runtime errors.
template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::mk_swinterval(Number n, wrapint::bitwidth_t width) {
  if (wrapint::fits_wrapint(n, width)) {
    //crab::outs() << "mk_swinterval1" << "\n";
    wrapint nw(n, width);
    CRAB_LOG("swrapped-imply-mk_swinterval", crab::outs() <<  "mk_swinterval of " 
              << n << "=" << swrapped_interval<Number>(nw) << "\n";);
    return swrapped_interval<Number>(nw);
  } else {
    CRAB_WARN(n, " does not fit into a wrapint. Returned top wrapped interval");
    CRAB_LOG("swrapped-imply-mk_swinterval", crab::outs() <<  "mk_swinterval of " 
              << n << "=" << swrapped_interval<Number>::top() << "\n";);
    return swrapped_interval<Number>::top();
  }
}

// Return top if lb or ub do not fit into a wrapint. No runtime errors.
template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::mk_swinterval(Number lb, Number ub,
                                       wrapint::bitwidth_t width) {
  if (!wrapint::fits_wrapint(lb, width)) {
    CRAB_WARN(lb,
              " does not fit into a wrapint. Returned top wrapped interval");
    return swrapped_interval<Number>::top();
  } else if (!wrapint::fits_wrapint(ub, width)) {
    CRAB_WARN(ub,
              " does not fit into a wrapint. Returned top wrapped interval");
    return swrapped_interval<Number>::top();
  } else {
    wrapint lbw(lb, width);
    wrapint ubw(ub, width);
    if(lbw.msb() ^ ubw.msb()){
      if(lbw.msb()){
        return swrapped_interval<Number>(wrapint::get_signed_max(width),
          wrapint::get_unsigned_min(width), true, lbw, ubw, false);
      }else{
        return swrapped_interval<Number>(lbw, ubw, false,
          wrapint::get_unsigned_max(width), wrapint::get_signed_min(width), true);
      }
    }else{
      return swrapped_interval<Number>(wrapint::get_unsigned_min(width), ubw, false,
        lbw, wrapint::get_unsigned_max(width), false);
    }
  }
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::top() {
  return swrapped_interval<Number>(wrapint(0, 3), wrapint(3, 3), false,
    wrapint(4, 3), wrapint(7, 3), false);
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::top(bitwidth_t width) {
  return swrapped_interval<Number>(wrapint::get_unsigned_min(width), wrapint::get_signed_max(width), false,
    wrapint::get_signed_min(width), wrapint::get_unsigned_max(width), false);
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::bottom() {
  return swrapped_interval<Number>(wrapint(1, 3), wrapint(0, 3), true,
    wrapint(7, 3), wrapint(6, 3), true);
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::bottom(bitwidth_t width) {
  return swrapped_interval<Number>(wrapint::get_signed_max(width), wrapint::get_unsigned_min(width), true,
   wrapint::get_unsigned_max(width), wrapint::get_signed_min(width), true);
}

template <typename Number>
wrapint::bitwidth_t swrapped_interval<Number>::get_bitwidth(int line) const {

  if(is_bottom() ){
    CRAB_ERROR("get_bitwidth() cannot be called from a bottom element at line ",
               line);
  }else if(is_top()){
    CRAB_ERROR("get_bitwidth() cannot be called from a top element at line ",
               line);
  }else{
    if(!is_bottom_0()){
      wrapint::bitwidth_t res =  m_start_0.get_bitwidth();
      //crab::outs() << "get_bitwidth0 = " << res <<"\n";
      return res;
    }else{
      wrapint::bitwidth_t res = m_start_1.get_bitwidth();
      //crab::outs() << "get_bitwidth1 =" << res <<"\n";
      return res;
    }
  }
  /*if((bottom0 || top0) && (bottom1 || top1)){
    CRAB_ERROR("get_bitwidth() cannot be called from all bottom/top element at line ",
               line);
  }else if((bottom0 || top0) && !(bottom1 || top1)){
    assert(m_end_1.get_bitwidth() == m_start_1.get_bitwidth());
    return m_start_1.get_bitwidth();
  }else if(!(bottom0 || top0) && (bottom1 || top1)){
    assert(m_end_0.get_bitwidth() == m_start_0.get_bitwidth());
    return m_start_0.get_bitwidth();
  }else{
    assert((m_end_0.get_bitwidth() == m_start_0.get_bitwidth())
            && (m_end_1.get_bitwidth() == m_start_1.get_bitwidth())
            && (m_end_0.get_bitwidth() == m_end_1.get_bitwidth()));
    return m_start_0.get_bitwidth();
  }*/
}

template <typename Number> wrapint swrapped_interval<Number>::start_0() const {
  if (is_bottom()) {
    CRAB_ERROR("method start_0() cannot be called if 0 circle bottom");
  }
  return m_start_0;
}

template <typename Number> wrapint swrapped_interval<Number>::end_0() const {
  if (is_bottom()) {
    CRAB_ERROR("method end() cannot be called if 0 circle top");
  }
  return m_end_0;
}

template <typename Number> wrapint swrapped_interval<Number>::start_1() const {
  if (is_bottom()) {
    CRAB_ERROR("method start_0() cannot be called if 1 circle bottom");
  }
  return m_start_1;
}

template <typename Number> wrapint swrapped_interval<Number>::end_1() const {
  if (is_bottom()) {
    CRAB_ERROR("method end() cannot be called if 1 circle top");
  }
  return m_end_1;
}

template <typename Number> void swrapped_interval<Number>::set_start_0(wrapint n){
  m_start_0 = n;
}

template <typename Number> void swrapped_interval<Number>::set_end_0(wrapint n){
  m_end_0 = n;
}

template <typename Number> void swrapped_interval<Number>::set_start_1(wrapint n){
  m_start_1 = n;
}

template <typename Number> void swrapped_interval<Number>::set_end_1(wrapint n){
  m_end_1 = n;
}

template <typename Number> void swrapped_interval<Number>::set_is_bottom_0(bool b){
  m_is_bottom_0 = b;
}

template <typename Number> void swrapped_interval<Number>::set_is_bottom_1(bool b){
  m_is_bottom_1 = b;
}

template <typename Number> void swrapped_interval<Number>::set_bottom0(){
  m_start_0 = wrapint(1, 3);
  m_end_0 = wrapint(0, 3);
  m_is_bottom_0 = true;
}

template <typename Number> void swrapped_interval<Number>::set_bottom1(){
  m_start_1 = wrapint(7, 3);
  m_end_1 = wrapint(6, 3);
  m_is_bottom_1 = true;
}

template <typename Number> void swrapped_interval<Number>::set_top0(){
  m_start_0 = wrapint(0, 3);
  m_end_0 = wrapint(3, 3);
  m_is_bottom_0 = false;
}

template <typename Number> void swrapped_interval<Number>::set_top1(){
  m_start_1 = wrapint(4, 3);
  m_end_1 = wrapint(7, 3);
  m_is_bottom_1 = false;
}

template <typename Number> void 
swrapped_interval<Number>::set_circle0(wrapint start, wrapint end, bool flag){
  m_start_0 = start;
  m_end_0 = end;
  m_is_bottom_0 = flag;
}
template <typename Number> void 
swrapped_interval<Number>::set_circle1(wrapint start, wrapint end, bool flag){
  m_start_1 = start;
  m_end_1 = end;
  m_is_bottom_1 = flag;
}

template <typename Number> bool swrapped_interval<Number>::is_bottom_0() const {
  if(m_is_bottom_0)
    return true ;
  if(m_start_0>m_end_0)
    return true;
  return false;
}

template <typename Number> bool swrapped_interval<Number>::is_bottom_1() const {
  if(m_is_bottom_1)
    return true ;
  if(m_start_1>m_end_1)
    return true;
  return false;
}

template <typename Number> bool swrapped_interval<Number>::is_bottom() const {
  //crab::outs() << "is_bottom" << "\n";
  if(m_is_bottom_0 && m_is_bottom_1) 
    return true;
  //crab::outs() << "1is_bottom" << "\n";
  if(m_start_0>m_end_0 && m_start_1>m_end_1)
    return true;
  //crab::outs() << "2is_bottom" << "\n";
  return false;
}

template <typename Number> bool swrapped_interval<Number>::is_top_0() const {
  //crab::outs() << "is_top_0" << "\n";
  wrapint::bitwidth_t width = m_start_0.get_bitwidth();
  //crab::outs() << "is_top_0" << "\n";
  // TODO:width=1?
  return (!m_is_bottom_0 && (m_end_0 - m_start_0 == wrapint::get_signed_max(width)));
}

template <typename Number> bool swrapped_interval<Number>::is_top_1() const {
  wrapint::bitwidth_t width = m_start_1.get_bitwidth();
  return (!m_is_bottom_1 && (m_end_1 - m_start_1 == wrapint::get_signed_max(width)));
}

template <typename Number> bool swrapped_interval<Number>::is_top() const {
  //crab::outs() << "is_top" << "\n";
  return is_top_0() && is_top_1();
}

template <typename Number> wrapint swrapped_interval<Number>::getSignedMaxValue() const {
  //wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  if(!is_bottom_0()){
    return m_end_0;
  }else if(!is_bottom_1()){
    return m_end_1;
  }else{
    CRAB_ERROR("method getSignedMaxValue() cannot be called if bottom");
  }
}

template <typename Number> wrapint swrapped_interval<Number>::getSignedMinValue() const {
  //wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  if(!is_bottom_1()){
    return m_start_1;
  }else if(!is_bottom_0()){
    return m_start_0;
  }else{
    CRAB_ERROR("method getSignedMinValue() cannot be called if bottom");
  }
}

template <typename Number> wrapint swrapped_interval<Number>::getUnsignedMaxValue() const {
  //wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  if(!is_bottom_1()){
    return m_end_1;
  }else if(!is_bottom_0()){
    return m_end_0;
  }else{
    CRAB_ERROR("method getUnsignedMaxValue() cannot be called if bottom");
  }
}

template <typename Number> wrapint swrapped_interval<Number>::getUnsignedMinValue() const {
  //wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  if(!is_bottom_0()){
    return m_start_0;
  }else if(!is_bottom_1()){
    return m_start_1;
  }else{
    CRAB_ERROR("method getUnsignedMinValue() cannot be called if bottom");
  }
}

template <typename Number>
ikos::interval<Number> swrapped_interval<Number>::to_interval() const {
  using interval_t = ikos::interval<Number>;
  if (is_bottom()) {
    return interval_t::bottom();
  } else if (is_top() || (m_end_0.is_signed_max() && m_start_1.is_signed_min())) {
    return interval_t::top();
  } else {
    interval_t i0(m_start_0.get_signed_bignum(), m_end_0.get_signed_bignum());
    interval_t i1(m_start_1.get_signed_bignum(), m_end_1.get_signed_bignum());
    return i0 | i1;
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::lower_half_line(bool is_signed) const {
  /*CRAB_LOG("swrapped-imply-lower_half_line", crab::outs() <<  "lower_half_line of " 
              << *this << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() <<  "lower_half_line of " 
              << *this << "\n";);  */          
  if (is_top() || is_bottom() )
    return *this;

  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  wrapint sign_max = wrapint::get_signed_max(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  wrapint unsign_min = wrapint::get_unsigned_min(b);

  if(is_signed){
    if(!is_bottom_0()){
      if(m_end_0 == sign_max){
        return swrapped_interval<Number>::top(b);
      }else{
        return swrapped_interval<Number>(wrapint(0, b), m_end_0, false,
          sign_min, unsign_max, false);
      }
    }else if (!is_bottom_1()){
      return swrapped_interval<Number>(sign_max, unsign_min, true,
          sign_min, m_end_1, false);
    }else{
      return swrapped_interval<Number>::bottom(b);
    }
    
  }else{
    if(!is_bottom_1()){
      if(m_end_1 == unsign_max){
        return swrapped_interval<Number>::top(b);
      }else{
        return swrapped_interval<Number>(unsign_min, sign_max, false,
          sign_min, m_end_1, false);
      }
    }else if(!is_bottom_0()){
      return swrapped_interval<Number>(wrapint(0, b), m_end_0, false,
          unsign_max, sign_min, false);
    }else{
      return swrapped_interval<Number>::bottom(b);
    }
  }
  
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::lower_half_line(const swrapped_interval_t &x, bool is_signed) const {
  swrapped_interval<Number> res = swrapped_interval<Number>::lower_half_line(is_signed);
  //CRAB_LOG("swrapped-imply-lower_half_line", crab::outs() <<  "lower_half_line of " << *this << "=" << res << "\n";);
  return res;
}




template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::upper_half_line(bool is_signed) const {
  if (is_top() || is_bottom())
    return *this;

  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  wrapint sign_max = wrapint::get_signed_max(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  wrapint unsign_min = wrapint::get_unsigned_min(b);

  if(is_signed){
    if(!is_bottom_1()){
      if(m_start_1 == sign_min){
        return swrapped_interval<Number>::top(b);
      }else{
        return swrapped_interval<Number>(unsign_min, sign_max, false,
          m_start_1, unsign_max, false);
      }
    }else if (!is_bottom_0()){
      return swrapped_interval<Number>(m_start_0, sign_max, false,
          unsign_max, sign_min, true);
    }else{
      return swrapped_interval<Number>::bottom(b);
    }
    
  }else{
    if(!is_bottom_0()){
      if(m_start_0 == unsign_min){
        return swrapped_interval<Number>::top(b);
      }else{
        return swrapped_interval<Number>(m_start_0, sign_max, false,
          sign_min, unsign_max, false);
      }
    }else if(!is_bottom_1()){
      return swrapped_interval<Number>(sign_max, unsign_min, true, 
        m_start_1, unsign_max, false);
    }else{
      return swrapped_interval<Number>::bottom(b);
    }
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::upper_half_line(const swrapped_interval_t &x, bool is_signed) const {
  //CRAB_LOG("swrapped-imply", crab::outs() <<  "upper_half_line of " << *this << "="  << "\n";);
   swrapped_interval<Number> res = swrapped_interval<Number>::upper_half_line(is_signed);
  //CRAB_LOG("swrapped-imply-upper_half_line", crab::outs() <<  "upper_half_line of " << *this << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() <<  "upper_half_line of " << *this << "=" << res << "\n";);
  return res;
}



template <typename Number> bool swrapped_interval<Number>::is_singleton() const {
  return (!is_bottom() && !is_top() && 
          ((m_start_0 == m_end_0) ^ (m_start_1 == m_end_1)));
}

template <typename Number> bool swrapped_interval<Number>::is_singleton_both_circle() const {
  return (!is_bottom() && !is_top() && 
          ((m_start_0 == m_end_0) && (m_start_1 == m_end_1)));
}



// Starting from m_start and going clock-wise x is encountered
// before than m_end.
template <typename Number> bool swrapped_interval<Number>::at(wrapint x) const {
  //CRAB_LOG("swrapped-imply-at", crab::outs() << x << "at " << *this << "?: " << "\n";);
  if(x.msb()){
    if(is_bottom_1()){
      return false;
    }else if(is_top_1()){
      return true;
    }else{
      return (m_start_1 < x) && (x < m_end_1);
    }
  }else{
    if(is_bottom_0()){
      return false;
    }else if(is_top_0()){
      return true;
    }else{
      return (m_start_0 < x) && (x < m_end_0);
    }
  }

}

template <typename Number>
bool swrapped_interval<Number>::operator<=(
    const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-inclusion", crab::outs() << *this <<  " operator<=  " << x << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this <<  " operator<=  " << x << "\n";);
  bool res0 = false;
  bool res1 = false;
  if (x.is_top_0() || is_bottom_0()) {
    res0 = true;
  } else if (x.is_bottom_0() || is_top_0()) {
    res0 = false;
  } else{
    res0 = (x.m_start_0 <= m_start_0) && (m_end_0 <= x.m_end_0);
  }

  if (x.is_top_1() || is_bottom_1()) {
    res1 = true;
  } else if (x.is_bottom_1() || is_top_1()) {
    res1 = false;
  } else{
    res1 = (x.m_start_1 <= m_start_1) && (m_end_1 <= x.m_end_1);
  }
  //CRAB_LOG("swrapped-imply-inclusion", crab::outs() << *this <<  " operator<=  " << x  << " = " <<( res0 && res1)<< "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this <<  " operator<=  " << x  << " = " <<( res0 && res1)<< "\n";);
  return res0 && res1;
}

template <typename Number>
bool swrapped_interval<Number>::operator==(
    const swrapped_interval<Number> &x) const {
  return (*this <= x && x <= *this);
}

template <typename Number>
bool swrapped_interval<Number>::operator!=(
    const swrapped_interval<Number> &x) const {
  return !(this->operator==(x));
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::operator|(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-join", crab::outs() << *this <<  " operator|  " << x << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this <<  " operator|  " << x << "\n";);
  if (is_bottom()) {
    return x;
  }
  if (x.is_bottom()) {
    return *this;
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  swrapped_interval<Number> res = swrapped_interval<Number>::top(width);
//crab::outs() << "operator|1" << "\n";
  if(is_bottom_0() && x.is_bottom_0()){
    res.set_start_0(wrapint::get_signed_max(width));
    res.set_end_0(wrapint::get_unsigned_min(width));
    res.set_is_bottom_0(true);
  }else if(is_bottom_0() && !x.is_bottom_0()){
    res.set_start_0(x.m_start_0);
    res.set_end_0(x.m_end_0);
    res.set_is_bottom_0(x.m_is_bottom_0);
  }else if(x.is_bottom_0() && !is_bottom_0()){
    res.set_start_0(m_start_0);
    res.set_end_0(m_end_0);
    res.set_is_bottom_0(m_is_bottom_0);
  }else if(!is_top_0() && !x.is_top_0()){
    res.set_start_0(wrapint::min(m_start_0, x.m_start_0));
    res.set_end_0(wrapint::max(m_end_0, x.m_end_0));
  }

   if(is_bottom_1() && x.is_bottom_1()){
    res.set_start_1(wrapint::get_unsigned_max(width));
    res.set_end_1(wrapint::get_signed_min(width));
    res.set_is_bottom_1(true);
  }else if(is_bottom_1()){
    res.set_start_1(x.m_start_1);
    res.set_end_1(x.m_end_1);
    res.set_is_bottom_1(x.m_is_bottom_1);
  }else if(x.is_bottom_1()){
    res.set_start_1(m_start_1);
    res.set_end_1(m_end_1);
    res.set_is_bottom_1(m_is_bottom_1);
  }else if(!is_top_1() && !x.is_top_1()){
    res.set_start_1(wrapint::min(m_start_1, x.m_start_1));
    res.set_end_1(wrapint::max(m_end_1, x.m_end_1));
  }
  //crab::outs() << "operator|2" << "\n";
  //CRAB_LOG("swrapped-imply-join", crab::outs() << *this << " U " << x << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " U " << x << "=" << res << "\n";);
  return res;
/*
  wrapint res_start_0 = wrapint::get_unsigned_min(width);
  wrapint res_end_0 = wrapint::get_signed_max(width);
  bool res_is_bottom_0 = false;
  wrapint res_start_1 = wrapint::get_signed_min(width);
  wrapint res_end_1 = wrapint::get_unsigned_max(width);
  bool res_is_bottom_1 = false;

  if(is_bottom_0()){
    res_start_0 = x.m_start_0;
    res_end_0 = x.m_end_0;
    res_is_bottom_0 = x.m_is_bottom_0;
  } else if (x.is_bottom_0()){
    res_start_0 = m_start_0;
    res_end_0 = m_end_0;
    res_is_bottom_0 = m_is_bottom_0;
  } else{
    res_start_0 = wrapint::min(m_start_0, x.m_start_0);
    res_end_0 =  wrapint::max(m_end_0, x.m_end_0);
    res_is_bottom_0 = false;
  }

  if(is_bottom_1()){
    res_start_1 = x.m_start_1;
    res_end_1 = x.m_end_1;
    res_is_bottom_1 = x.m_is_bottom_1;
  } else if (x.is_bottom_1()){
    res_start_1 = m_start_1;
    res_end_1 = m_end_1;
    res_is_bottom_1 = m_is_bottom_1;
  } else{
    res_start_1 = wrapint::min(m_start_1, x.m_start_1);
    res_end_1 = wrapint::max(m_end_1, x.m_end_1);
    res_is_bottom_1 = false;
  }

  return swrapped_interval<Number>(res_start_0, res_end_0, res_is_bottom_0, 
            res_start_1, res_end_1, res_is_bottom_1);
  */
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::operator&(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-meet", crab::outs() << *this <<  " operator&  " << x << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this <<  " operator&  " << x << "\n";);
  swrapped_interval<Number> res = swrapped_interval<Number>::bottom();
  if(!is_bottom_0() && !x.is_bottom_0()){
    if(is_top_0()){
      res.set_circle0(x.m_start_0, x.m_end_0, x.m_is_bottom_0);
    }else if(x.is_top_0()){
      res.set_circle0(m_start_0, m_end_0, m_is_bottom_0);
    }else{
      res.set_start_0(wrapint::max(m_start_0, x.m_start_0));
      res.set_end_0(wrapint::min(m_end_0, x.m_end_0));
      if(res.m_start_0 <= res.m_end_0){
        res.set_is_bottom_0(false);
      }
    }
  }

  if(!is_bottom_1() && !x.is_bottom_1()){
    if(is_top_1()){
      res.set_circle1(x.m_start_1, x.m_end_1, x.m_is_bottom_1);
    }else if(x.is_top_1()){
      res.set_circle1(m_start_1, m_end_1, m_is_bottom_1);
    }else{
      res.set_start_1(wrapint::max(m_start_1, x.m_start_1));
      res.set_end_1(wrapint::min(m_end_1, x.m_end_1));
      if(res.m_start_1 <= res.m_end_1){
        res.set_is_bottom_1(false);
      }
    }
  }
  //CRAB_LOG("swrapped-int-meet", crab::outs() << "start = " << wrapint::max(m_start_1, x.m_start_1) <<  ", end = " <<(wrapint::min(m_end_1, x.m_end_1))<< "\n");
  //CRAB_LOG("swrapped-int-meet", crab::outs() << "res.start = " << res.m_start_1 <<  ", res.end = " << res.m_end_1 << ", res.is_bottom = "<< res.m_is_bottom_1 << "\n");
  //CRAB_LOG("swrapped-imply-meet", crab::outs() << *this << " n " << x << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " n " << x << "=" << res << "\n";);
  return res;

/*
  wrapint::bitwidth_t width = get_bitwidth(__LINE__);

  wrapint res_start_0 = wrapint::get_signed_max(width);
  wrapint res_end_0 = wrapint::get_unsigned_min(width);
  bool res_is_bottom_0 = true;
  wrapint res_start_1 = wrapint::get_unsigned_max(width);
  wrapint res_end_1 = wrapint::get_signed_min(width);
  bool res_is_bottom_1 = true;

  if(!is_bottom_0() && !x.is_bottom_0()){
    res_start_0 = wrapint::max(m_start_0, x.m_start_0);
    res_end_0 = wrapint::min(m_end_0, x.m_end_0);
    res_is_bottom_0 = false;
  }

  if(!is_bottom_1() && !x.is_bottom_1()){
    res_start_1 = wrapint::max(m_start_1, x.m_start_1);
    res_end_1 = wrapint::min(m_end_1, x.m_end_1);
    res_is_bottom_1 = false;
  }

  return swrapped_interval<Number>(res_start_0, res_end_0, res_is_bottom_0, 
            res_start_1, res_end_1, res_is_bottom_1);*/
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::operator||(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-widen", crab::outs() << *this << " || " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " || " << x << " = "  << "\n";);
  if (is_bottom()) {
    return x;
  }
  if (x.is_bottom()) {
    return *this;
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  wrapint sign_max = wrapint::get_signed_max(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  wrapint unsign_min = wrapint::get_unsigned_min(b);

  wrapint res_start0 = unsign_min;
  wrapint res_end0 = sign_max;
  wrapint res_start1 = sign_min;
  wrapint res_end1 = unsign_max;


  if (is_bottom_0() && !x.is_bottom_0()) {
    res_start0 = x.m_start_0;
    res_end0 = x.m_end_0;
  } else if (x.is_bottom_0() && !is_bottom_0()) {
    res_start0 = m_start_0;
    res_end0 = m_end_0;
  } else if(is_bottom_0() && x.is_bottom_0()){
    res_start0 = sign_max;
    res_end0 = unsign_min;
  }else if (is_top_0() || x.is_top_0()) {
    ;
  } else if (x.m_start_0 >= m_start_0 && x.m_end_0 <= m_end_0) {
    res_start0 = m_start_0;
    res_end0 = m_end_0;
  }else{
    assert(get_bitwidth(__LINE__) == x.get_bitwidth(__LINE__));


    wrapint join_start0 = wrapint::min(m_start_0, x.m_start_0);
    wrapint join_end0 = wrapint::max(m_end_0, x.m_end_0);
    /*CRAB_LOG("swrapped-imply-widen", crab::outs() << "join_start0 = " 
            << join_start0 << ", join_end0 = " << join_end0 << "\n";);*/

    if(join_start0 == m_start_0 && join_end0 == x.m_end_0){
      wrapint new_end0 = (m_end_0 * wrapint(2, b)) - m_start_0 + wrapint(1, b);
      res_start0 = m_start_0;
      if(new_end0 >= sign_max){
        res_end0 = sign_max;
      }else{
        res_end0 = wrapint::max(join_end0, new_end0);
      }
    }else if(join_start0 == x.m_start_0 && join_end0 == m_end_0){
      wrapint new_start0 = (m_start_0 * wrapint(2, b)) - m_end_0 - wrapint(1, b);
      res_end0 = m_end_0;
      if(new_start0>=sign_max){
        res_start0 = unsign_min;
      }else{
        res_start0 = wrapint::min(join_start0, new_start0);
      }
    }else if(x.m_start_0< m_start_0 && x.m_end_0>m_end_0){
      wrapint new_end0 = (m_end_0 -m_start_0)*wrapint(2, b) + wrapint(1, b);
      res_start0 = x.m_start_0;
      if(new_end0 >= sign_max){
        res_end0 = sign_max;
      }else{
        res_end0 = wrapint::max(x.m_end_0, new_end0);
      }
    }
  }
  /*CRAB_LOG("swrapped-imply-widen", crab::outs() << "res_start0 = " 
            << res_start0 << ", res_end0 = " << res_end0 << "\n";);*/

  if (is_bottom_1() && !x.is_bottom_1()) {
    res_start1 = x.m_start_1;
    res_end1 = x.m_end_1;
  } else if (x.is_bottom_1() && !is_bottom_1()) {
    res_start1 = m_start_1;
    res_end1 = m_end_1;
  } else if(is_bottom_1() && x.is_bottom_1()){
    res_start1 = unsign_max;
    res_end1 = sign_min;
  }else if (is_top_1() || x.is_top_1()) {
    ;
  } else if (x.m_start_1 >= m_start_1 && x.m_end_1 <= m_end_1) {
    res_start1 = m_start_1;
    res_end1 = m_end_1;
  }else{
    assert(get_bitwidth(__LINE__) == x.get_bitwidth(__LINE__));


    wrapint join_start1 = wrapint::min(m_start_1, x.m_start_1);
    wrapint join_end1 = wrapint::max(m_end_1, x.m_end_1);
   /*CRAB_LOG("swrapped-imply-widen", crab::outs() << "join_start1 = " 
            << join_start1 << ", join_end1 = " << join_end1 << "\n";);*/ 

    if(join_start1 == m_start_1 && join_end1 == x.m_end_1){
      wrapint new_end1 = (m_end_1 * wrapint(2, b)) - m_start_1 + wrapint(1, b);
      res_start1 = m_start_1;
      if(new_end1 <= sign_max){
        res_end1 = unsign_max;
      }else{
        res_end1 = wrapint::max(join_end1, new_end1);
      }
    }else if(join_start1 == x.m_start_1 && join_end1 == m_end_1){
      wrapint new_start1 = (m_start_1 * wrapint(2, b)) - m_end_1 - wrapint(1, b);
      res_end1 = m_end_1;
      if(new_start1 <= sign_max){
        res_start1 = sign_min;
      }else{
        res_start1 = wrapint::min(join_start1, new_start1);
      }
    }else if(x.m_start_1< m_start_1 && x.m_end_1>m_end_1){
      wrapint new_end1 = (m_end_1 -m_start_1)*wrapint(2, b) + wrapint(1, b);
      res_start1 = x.m_start_1;
      if(new_end1 <= sign_max){
        res_end1 = unsign_max;
      }else{
        res_end1 = wrapint::max(x.m_end_1, new_end1);
      }
    }
  }
  /*CRAB_LOG("swrapped-imply-widen", crab::outs() << "res_start1 = " 
            << res_start1 << ", res_end1 = " << res_end1 << "\n";);*/

  swrapped_interval<Number> res(res_start0, res_end0, res_start1, res_end1);
  //CRAB_LOG("swrapped-imply-widen", crab::outs() << *this << " || " << x << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " || " << x << "=" << res << "\n";);
  return res;
  
}

// TODO: factorize code with operator||
template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::widening_thresholds(
    const swrapped_interval<Number> &x, const thresholds<Number> &ts) const {
  //CRAB_LOG("swrapped-imply-widening_thresholds", crab::outs() << "widening_thresholds of " << *this <<  "  and " << x << "\n";);
  return (this->operator||(x));
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::operator&&(const swrapped_interval<Number> &x) const {
  // TODO: for now we call the meet operator.
   //CRAB_LOG("swrapped-imply-narrow", crab::outs() << "narrowing of " << *this <<  "  and " << x << "\n";);
  return (this->operator&(x));
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::operator+(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-add", crab::outs() << *this << " + " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " + " << x << " = "  << "\n";);
  if (is_bottom() || x.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  wrapint sign_max = wrapint::get_signed_max(width);
  wrapint sign_min = wrapint::get_signed_min(width);
  wrapint unsign_max = wrapint::get_unsigned_max(width);
  wrapint unsign_min = wrapint::get_unsigned_min(width);

  swrapped_interval_t res00 = swrapped_interval<Number>::bottom();
  swrapped_interval_t res01 = swrapped_interval<Number>::bottom();
  swrapped_interval_t res10 = swrapped_interval<Number>::bottom();
  swrapped_interval_t res11 = swrapped_interval<Number>::bottom();

  bool t0_bottom = is_bottom_0();
  bool t1_bottom = is_bottom_1();
  bool x0_bottom = x.is_bottom_0();
  bool x1_bottom = x.is_bottom_1(); 

  bool t0_top = is_top_0();
  bool t1_top = is_top_1();
  bool x0_top = x.is_top_0();
  bool x1_top = x.is_top_1();


  if(!t0_bottom && !x0_bottom){
    if(t0_top && !x0_top){
      res00 = split(x.m_start_0, x.m_end_0 + sign_max);
    }else if(!t0_top && x0_top){
      res00 = split(m_start_0, m_end_0 + sign_max);
    }else if(t0_top && x0_top){
      res00 = swrapped_interval<Number>(unsign_min, sign_max, false, sign_min, (unsign_max - wrapint(2, width)), false); 
      //res00 = swrapped_interval<Number>::top(); 
    }else{
      wrapint start00 = m_start_0 + x.m_start_0;
      wrapint end00 = m_end_0 + x.m_end_0;
      res00 = split(start00, end00);
    }
  }

  if(!t0_bottom && !x1_bottom){
    if(t0_top && !x1_top){
      res01 = split(x.m_start_1, x.m_end_1 + sign_max);
    }else if(!t0_top && x1_top){
      res01 = split(m_start_0 + sign_min, m_end_0 + unsign_max);
    }else if(t0_top && x1_top){
      res01 = swrapped_interval<Number>(unsign_min, (sign_max - wrapint(1, width)), false, sign_min, unsign_max, false);
    }else{
      wrapint start01 = m_start_0 + x.m_start_1;
      wrapint end01 = m_end_0 + x.m_end_1;
      res01 = split(start01, end01);    
    }
  }

  if(!t1_bottom && !x0_bottom){
    if(t1_top && !x0_top){
      //res10 = split(x.m_start_1 + sign_min, x.m_end_1 + unsign_max); // 
      res10 = split(x.m_start_0 + sign_min, x.m_end_0 + unsign_max);
    }else if(!t1_top && x0_top){
      res10 = split(m_start_0, m_end_0 + sign_max);
    }else if(t1_top && x0_top){
      res10 = swrapped_interval<Number>(unsign_min, (sign_max - wrapint(1, width)), false, sign_min, unsign_max, false);
    }else{
      wrapint start10 = m_start_1 + x.m_start_0;
      wrapint end10 = m_end_1 + x.m_end_0;
      res10 = split(start10, end10);
    }
  }

  if(!t1_bottom && !x1_bottom){
    if(t1_top && !x1_top){
      res11 = split(x.m_start_1 + sign_min, x.m_end_1 + unsign_max);
    }else if(!t1_top && x1_top){
      res11 = split(m_start_0 + sign_min, m_end_0 + unsign_max);
    }else if(t1_top && x1_top){
      res11 = swrapped_interval<Number>(unsign_min, sign_max, false, sign_min, unsign_max - wrapint(1, width), false);
    }else{
      wrapint start11 = m_start_1 + x.m_start_1;
      wrapint end11 = m_end_1 + x.m_end_1;
      res11 = split(start11, end11);
    }
  }

  swrapped_interval<Number> res = res00 | res01 | res10 | res11;
  //CRAB_LOG("swrapped-imply-add", crab::outs() << *this << " + " << x << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " + " << x << "=" << res << "\n";);
  return res;
  /*
  wrapint start00 = m_start_0 + x.m_start_0;
  wrapint start01 = m_start_0 + x.m_start_1;
  wrapint end00 = m_end_0 + x.m_end_0;
  wrapint end01 = m_end_0 + x.m_end_1;
  wrapint start10 = m_start_1 + x.m_start_0;
  wrapint start11 = m_start_1 + x.m_start_1;
  wrapint end10 = m_end_1 + x.m_end_0;
  wrapint end11 = m_end_1 + x.m_end_1;

  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  swrapped_interval_t res00 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res01 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res10 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res11 = swrapped_interval<Number>::bottom(b);
  wrapint sign_max = wrapint::get_signed_max(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  //wrapint unsign_min = wrapint::get_unsigned_min(b);

  bool t0_bottom = is_bottom_0();
  bool t1_bottom = is_bottom_1();
  bool x0_bottom = x.is_bottom_0();
  bool x1_bottom = x.is_bottom_1();

  if(!t0_bottom && !x0_bottom){
    if(end00 <= sign_max) {
      res00 = swrapped_interval<Number>(start00, end00, false,
              unsign_max, sign_min, true);
    }else{
      res00 = swrapped_interval<Number>(start00, sign_max, false,
              sign_min, end00, false);
    } 
  }

  if(!t1_bottom && !x1_bottom){
    if(end11 <= sign_max) {
      res11 = swrapped_interval<Number>(start11, end11, false,
              unsign_max, sign_min, true);
    }else{
      res11 = swrapped_interval<Number>(start11, sign_max, false,
              sign_min, end00, false);
    }
  }

  if(!t1_bottom && !x0_bottom){
    if(end10 >= sign_min) {
      res10 = swrapped_interval<Number>(start10, end10, false,
              unsign_max, sign_min, true);
    }else{
      res10 = swrapped_interval<Number>(start10, sign_max, false,
              sign_min, end10, false);
    } 
  }

  if(!t0_bottom && !x1_bottom){
    if(end01 >= sign_min) {
      res01 = swrapped_interval<Number>(start01, end01, false,
              unsign_max, sign_min, true);
    }else{
      res01 = swrapped_interval<Number>(start01, sign_max, false,
              sign_min, end01, false);
    } 
  }

  return res00 | res01 | res10 | res11;
*/
}

template <typename Number>
swrapped_interval<Number> &
swrapped_interval<Number>::operator+=(const swrapped_interval<Number> &x) {
  return this->operator=(this->operator+(x));
}

template <typename Number>
swrapped_interval<Number> swrapped_interval<Number>::operator-() const {
  //CRAB_LOG("swrapped-imply-neg", crab::outs() <<" - "  << *this   << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() <<" - "  << *this   << "\n";);
  if (is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top()) {
    return swrapped_interval<Number>::top();
  } 

  
  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  wrapint sign_max = wrapint::get_signed_max(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  wrapint unsign_min = wrapint::get_unsigned_min(b);
  swrapped_interval_t res0 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res1 = swrapped_interval<Number>::bottom(b);


  bool at_sign_min = at(sign_min);
  bool at_unsign_min = at(unsign_min);

  if(!is_bottom_0()){
    if(is_top_0()){
      res0 = swrapped_interval<Number>(unsign_min, unsign_min, false,
              sign_min + wrapint(1, b), unsign_max, false);
    }else if(m_start_0 == unsign_min){
      if(m_start_0 == m_end_0){
        res0 = swrapped_interval<Number>(unsign_min, unsign_min, false,
              unsign_max, sign_min, true);
      }else{
        res0 = swrapped_interval<Number>(unsign_min, unsign_min, false,
              -m_end_0, sign_max, false);
      }
    }else{
      res0 = swrapped_interval<Number>(sign_max, unsign_min, true,
              -m_end_0, -m_start_0, false);
    }
  }
  
  if(!is_bottom_1()){
    if(is_top_1()){
      res1 = swrapped_interval<Number>(unsign_min + wrapint(1, b), sign_max, false,
              sign_min, sign_min, false);
    }else if(sign_min == m_start_1){
      if(m_start_1 == m_end_1){
        res1 = swrapped_interval<Number>(sign_max, unsign_min, true,
              sign_min, sign_min, false);
      }else{
        res1 = swrapped_interval<Number>(-m_end_1, sign_max, false,
              sign_min, sign_min, false);
      }
    }else{
      res1 = swrapped_interval<Number>(-m_end_1, -m_start_1, false,
              unsign_max, sign_min, true);
    }
  }
  swrapped_interval<Number> res = res0 | res1;
  //CRAB_LOG("swrapped-imply-neg", crab::outs() << " - " << *this <<  "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << " - " << *this <<  "=" << res << "\n";);
  return res;
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::operator-(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-sub", crab::outs() << *this << " - " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " - " << x << " = "  << "\n";);
  if (is_bottom() || x.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  
  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  swrapped_interval_t res00 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res01 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res10 = swrapped_interval<Number>::bottom(b);
  swrapped_interval_t res11 = swrapped_interval<Number>::bottom(b);
  wrapint sign_max = wrapint::get_signed_max(b);
  wrapint sign_min = wrapint::get_signed_min(b);
  wrapint unsign_max = wrapint::get_unsigned_max(b);
  wrapint unsign_min = wrapint::get_unsigned_min(b);

  bool t0_bottom = is_bottom_0();
  bool t1_bottom = is_bottom_1();
  bool x0_bottom = x.is_bottom_0();
  bool x1_bottom = x.is_bottom_1();

  if(!t0_bottom && !x0_bottom){
    wrapint lb00 = m_start_0 - x.m_end_0;
    wrapint ub00 = m_end_0 - x.m_start_0;
    res00 = split(lb00, ub00);
    /*if(lb00 >= sign_min && ub00 <=sign_max) {
      res00 = swrapped_interval<Number>(unsign_min, ub00, false,
              lb00, unsign_max, false);
    }else if(lb00 >= sign_min && ub00 >= sign_min){
      res00 = swrapped_interval<Number>(sign_max, unsign_min, true,
              lb00, ub00, false);
    }else{
      res00 = swrapped_interval<Number>(lb00, ub00, false,
              unsign_max, sign_min, true);
    }*/
  }

  if(!t1_bottom && !x1_bottom){
    wrapint lb11 = m_start_1 - x.m_end_1;
    wrapint ub11 = m_end_1 - x.m_start_1;
    res11 = split(lb11, ub11);
    /*if(lb11 > sign_min && ub11 > sign_min) {
      res11 = swrapped_interval<Number>(sign_max, unsign_min, true,
              lb11, ub11, false);
    }else if(lb11 <= sign_max && ub11 <= sign_max){
      res11 = swrapped_interval<Number>(lb11, ub11, false,
              unsign_max, sign_min, true);
    }else{
      res11 = swrapped_interval<Number>(unsign_min, ub11, false,
              lb11, unsign_max, false);
    }*/
  }

  if(!t1_bottom && !x0_bottom){
    wrapint lb10 = m_start_1 - x.m_end_0;
    wrapint ub10 = m_end_1 - x.m_start_0;
    res10 = split(lb10, ub10);
  }

  if(!t0_bottom && !x1_bottom){
    wrapint lb01 = m_start_0 - x.m_end_1;
    wrapint ub01 = m_end_0 - x.m_start_1;
    res01 = split(lb01, ub01);
    /*if(lb01 <= sign_max && ub01 <= sign_max) {
      res01 = swrapped_interval<Number>(lb01, ub01, false,
              unsign_max, sign_min, true);
    }else if(lb01 >= sign_min && ub01 >=sign_min){
      res01 = swrapped_interval<Number>(sign_max, unsign_min, true,
              lb01, ub01, false);
    }else{
      res01 = swrapped_interval<Number>(lb01, sign_max, true,
              sign_min, ub01, false);
    }*/
  }

    swrapped_interval<Number> res = res00 | res01 | res10 | res11;
  //CRAB_LOG("swrapped-imply-sub", crab::outs() << *this << " - " << x << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " - " << x << "=" << res << "\n";);
  return res;
}

template <typename Number>
swrapped_interval<Number> &
swrapped_interval<Number>::operator-=(const swrapped_interval<Number> &x) {
  return this->operator=(this->operator-(x));
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::operator*(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-mul", crab::outs() << *this << " * " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " * " << x << " = "  << "\n";);
  if (is_bottom() || x.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  //CRAB_LOG("swrapped-imply-mul", crab::outs() << *this << " * " << x << ":" << "\n";);
  wrapint::bitwidth_t b = get_bitwidth(__LINE__);
  swrapped_interval_t res00 = swrapped_interval<Number>::bottom();
  swrapped_interval_t res01 = swrapped_interval<Number>::bottom();
  swrapped_interval_t res10 = swrapped_interval<Number>::bottom();
  swrapped_interval_t res11 = swrapped_interval<Number>::bottom();

  bool t0_bottom = is_bottom_0();
  bool t1_bottom = is_bottom_1();
  bool x0_bottom = x.is_bottom_0();
  bool x1_bottom = x.is_bottom_1();
 /*CRAB_LOG("swrapped-imply-mul", crab::outs() << "t0_bottom = " << t0_bottom << ", " <<  
              "t1_bottom = " << t1_bottom << ", " <<
              "x0_bottom = "<< x0_bottom << ", "  << 
              "x1_bottom = " <<x1_bottom <<"\n";);*/

  if(!t0_bottom && !x0_bottom){
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "for res00: " << "\n";);
    res00 = reduced_signed_unsigned_mul(m_start_0, m_end_0, x.m_start_0, x.m_end_0);
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "res00 = "<< res00 << "\n";);
  }  

  if(!t0_bottom && !x1_bottom){
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "for res01: " << "\n";);
    res01 = reduced_signed_unsigned_mul(m_start_0, m_end_0, x.m_start_1, x.m_end_1);
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "res00 = "<< res01 << "\n";);
  }

  if(!t1_bottom && !x0_bottom){
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "for res10: " << "\n";);
    res10 = reduced_signed_unsigned_mul(m_start_1, m_end_1, x.m_start_0, x.m_end_0);
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "res10 = "<< res10 << "\n";);
  }

  if(!t1_bottom && !x1_bottom){
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "for res11: " << "\n";);
    res11 = reduced_signed_unsigned_mul(m_start_1, m_end_1, x.m_start_1, x.m_end_1);
    //CRAB_LOG("swrapped-imply-mul", crab::outs() << "res11 = "<< res11 << "\n";);
  }

    swrapped_interval<Number> res = res00 | res01 | res10 | res11;
  //CRAB_LOG("swrapped-imply-mul", crab::outs() << *this << " * " << x << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " * " << x << "=" << res << "\n";);
  return res;
}

template <typename Number>
swrapped_interval<Number> &
swrapped_interval<Number>::operator*=(const swrapped_interval<Number> &x) {
  return this->operator=(this->operator*(x));
}

/** division and remainder operations **/
template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::SDiv(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-sdiv", crab::outs() << *this << " /s " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " /s " << x << " = "  << "\n";);
  if (is_bottom() || x.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  swrapped_interval<Number> trimed = x.trim_zero();

    swrapped_interval<Number> res = signed_div(trimed);
  //CRAB_LOG("swrapped-imply-sdiv", crab::outs() << *this << " SDIV " << x << "=" << res << "\n";);
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " SDIV " << x << "=" << res << "\n";);
  return res;
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::UDiv(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-udiv", crab::outs() << *this << " /u " << x << " = "  << "\n";);
  if (is_bottom() || x.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  swrapped_interval<Number> trimed =  x.trim_zero();
  return unsigned_div(trimed);
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::SRem(const swrapped_interval<Number> &x) const { // 
  swrapped_interval<Number> trimed =  x.trim_zero();
  if (is_bottom() || trimed.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 
  
  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  swrapped_interval_t tmpRes = swrapped_interval<Number>::bottom(width);
  wrapint sign_min = wrapint::get_signed_min(width);
  wrapint unsign_max = wrapint::get_unsigned_max(width);

  swrapped_interval<Number> divRes = SDiv(trimed);
  if(divRes.is_singleton()){
    return (*this - divRes * x);
  }else{
    wrapint zero(0, width);
    if(trimed.is_bottom_0() && !trimed.is_bottom_1()){
      tmpRes = swrapped_interval<Number>(zero, -trimed.m_start_1, false, unsign_max, sign_min, true);
    }else if(!trimed.is_bottom_0() && trimed.is_bottom_1()){
      tmpRes = swrapped_interval<Number>(zero, trimed.m_end_0, false, unsign_max, sign_min, true);
    }else{
      wrapint startmax = trimed.m_end_0 > (-trimed.m_start_1) ? trimed.m_end_0 : (-trimed.m_start_1);
      tmpRes = swrapped_interval<Number>(zero, startmax, false, unsign_max, sign_min, true);
    }

    if(is_bottom_0() && !is_bottom_1()){
      return -tmpRes;
    }else if(!is_bottom_0() && is_bottom_1()){
      return tmpRes;
    }else{
      return tmpRes | (-tmpRes);
    }
  }
}


template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::URem(const swrapped_interval<Number> &x) const { // 
  swrapped_interval<Number> trimed =  x.trim_zero();
  if (is_bottom() || trimed.is_bottom()) {
    return swrapped_interval<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return swrapped_interval<Number>::top();
  } 

  swrapped_interval<Number> divRes = UDiv(trimed);
  if(divRes.is_singleton()){
    return (*this - divRes * x);
  }else{
    bitwidth_t width = get_bitwidth(__LINE__);
    wrapint zero(0, width);
    swrapped_interval<Number> resRange =  split(zero, trimed.getUnsignedMaxValue());
    return resRange;
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::ZExt(unsigned bits_to_add) const {
  //CRAB_LOG("swrapped-imply-zext", crab::outs() << *this << " zext " << bits_to_add << " = "  << "\n";);
 CRAB_LOG("swrapped-imply", crab::outs() << *this << " zext " << bits_to_add << " = "  << "\n";);
  if (is_bottom() || is_top()) {
    return *this;
  }
  bitwidth_t new_width = get_bitwidth(__LINE__) + bits_to_add;
  swrapped_interval<Number> res = swrapped_interval<Number>::bottom(new_width);
  if(!is_bottom_0()){
    res = res | swrapped_interval<Number>(m_start_0.zext(bits_to_add), m_end_0.zext(bits_to_add), false,
                  wrapint::get_unsigned_max(new_width), wrapint::get_signed_min(new_width), true);
  }
  if(!is_bottom_1()){
    res = res | swrapped_interval<Number>(m_start_1.zext(bits_to_add), m_end_1.zext(bits_to_add), false,
                  wrapint::get_unsigned_max(new_width), wrapint::get_signed_min(new_width), true);
  } 
  return res;
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::SExt(unsigned bits_to_add) const {
  //CRAB_LOG("swrapped-imply-sext", crab::outs() << *this << " sext " << bits_to_add << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " sext " << bits_to_add << " = "  << "\n";);
  if (is_bottom() || is_top()) {
    return *this;
  }
  bitwidth_t new_width = get_bitwidth(__LINE__) + bits_to_add;
  swrapped_interval<Number> res = swrapped_interval<Number>::bottom(new_width);
  if(!is_bottom_0()){
    res = res | swrapped_interval<Number>(m_start_0.sext(bits_to_add), m_end_0.sext(bits_to_add), false,
                  wrapint::get_unsigned_max(new_width), wrapint::get_signed_min(new_width), true);
  }
  if(!is_bottom_1()){
    res = res | swrapped_interval<Number>(wrapint::get_signed_max(new_width), wrapint::get_unsigned_min(new_width), true,
                m_start_1.sext(bits_to_add), m_end_1.sext(bits_to_add), false);
  } 
  return res;
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::Trunc(unsigned bits_to_keep) const {
  //CRAB_LOG("swrapped-imply-trunc", crab::outs() << *this << " trunc " << bits_to_keep << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " trunc " << bits_to_keep << " = "  << "\n";);
  if (is_bottom() || is_top()) {
    return *this;
  }
  swrapped_interval<Number> res0 = swrapped_interval<Number>::top(bits_to_keep);
  swrapped_interval<Number> res1 = swrapped_interval<Number>::top(bits_to_keep);

  wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  if (m_start_0.ashr(wrapint(bits_to_keep, w)) ==
      m_end_0.ashr(wrapint(bits_to_keep, w))) {
    wrapint lower_start = m_start_0.keep_lower(bits_to_keep);
    wrapint lower_end = m_end_0.keep_lower(bits_to_keep);
    if (lower_start <= lower_end) {
      res0 = split(lower_start, lower_end);
    }
  } else {
    // note that m_start is a wrapint so after +1 it can wraparound
    wrapint y(m_start_0.ashr(wrapint(bits_to_keep, w)));
    ++y;
    if (y == m_end_0.ashr(wrapint(bits_to_keep, w))) {
      wrapint lower_start = m_start_0.keep_lower(bits_to_keep);
      wrapint lower_end = m_end_0.keep_lower(bits_to_keep);
      if (!(lower_start <= lower_end)) {
        res0 = split(lower_start, lower_end);
      }
    }
  }

  if (m_start_1.ashr(wrapint(bits_to_keep, w)) ==
      m_end_1.ashr(wrapint(bits_to_keep, w))) {
    wrapint lower_start = m_start_1.keep_lower(bits_to_keep);
    wrapint lower_end = m_end_1.keep_lower(bits_to_keep);
    if (lower_start <= lower_end) {
      res0 = split(lower_start, lower_end);
    }
  } else {
    // note that m_start is a wrapint so after +1 it can wraparound
    wrapint y(m_start_1.ashr(wrapint(bits_to_keep, w)));
    ++y;
    if (y == m_end_1.ashr(wrapint(bits_to_keep, w))) {
      wrapint lower_start = m_start_1.keep_lower(bits_to_keep);
      wrapint lower_end = m_end_1.keep_lower(bits_to_keep);
      if (!(lower_start <= lower_end)) {
        res1 = split(lower_start, lower_end);
      }
    }
  }
  //CRAB_LOG("swrapped-imply", crab::outs() << *this << " trunc " << bits_to_keep << " = " << (res0 | res1) << "\n";);
  return res0 | res1;
}

/// Shl, LShr, and AShr shifts are treated as unsigned numbers
template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::Shl(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-shl", crab::outs() << *this << " shl " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " shl " << x << " = "  << "\n";);
  if (is_bottom())
    return *this;
  if(is_top()) // need to fix
    return *this;

  // only if shift is constant
  if (x.is_singleton()) {
    return Shl(x.start_0().get_uint64_t());
  } else {
    return swrapped_interval<Number>::top(get_bitwidth(__LINE__));
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::LShr(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-lshr", crab::outs() << *this << " lshr " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " lshr " << x << " = "  << "\n";);
  if (is_bottom())
    return *this;
  if(is_top()) // need to fix
    return *this;

  // only if shift is constant
  if (x.is_singleton()) {
    return LShr(x.start_0().get_uint64_t());
  } else {
    return swrapped_interval<Number>::top(get_bitwidth(__LINE__));
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::AShr(const swrapped_interval<Number> &x) const {
  //CRAB_LOG("swrapped-imply-ashr", crab::outs() << *this << " ashr " << x << " = "  << "\n";);
  CRAB_LOG("swrapped-imply", crab::outs() << *this << " ashr " << x << " = "  << "\n";);
  if (is_bottom()) {
    return *this;
  }
  if(is_top()) // need to fix
    return *this;

  // only if shift is constant
  if (x.is_singleton()) {
    return AShr(x.getUnsignedMaxValue().get_uint64_t());
  }  else {
    return swrapped_interval<Number>::top(get_bitwidth(__LINE__));
  }
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::And(const swrapped_interval<Number> &x) const {
  return default_implementation(x);
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::Or(const swrapped_interval<Number> &x) const {
  return default_implementation(x);
}

template <typename Number>
swrapped_interval<Number>
swrapped_interval<Number>::Xor(const swrapped_interval<Number> &x) const {
  return default_implementation(x);
}

template <typename Number>
void swrapped_interval<Number>::write(crab::crab_os &o) const {
  if (is_bottom()) {
    o << "_|_";
  } else if (is_top()) {
    o << "top";
  } else {
#ifdef PRINT_WRAPINT_AS_SIGNED
  // print the wrapints as a signed number (easier to read)
  uint64_t x0 = m_start_0.get_uint64_t();
  uint64_t y0 = m_end_0.get_uint64_t();
  uint64_t x1 = m_start_1.get_uint64_t();
  uint64_t y1 = m_end_1.get_uint64_t();

  o << "{";

  if(is_bottom_0()){
    o << "_|_";
  }else if(is_top_0()){
    o << "top";
  }else{
    if (get_bitwidth(__LINE__) == 32) {
      o << "[[" << (int)x0 << ", " << (int)y0 << "]]";
    } else if (get_bitwidth(__LINE__) == 8) {
      o << "[[" << (int)static_cast<signed char>(x0) << ", "
        << (int)static_cast<signed char>(y0) << "]]";
    } else {
      o << "[[" << m_start_0.get_signed_bignum() << ", "
        << m_end_0.get_signed_bignum() << "]]";
    }
  }

  o << ", ";

  if(is_bottom_1()){
    o << "_|_";
  }else if(is_top_1()){
    o << "top";
  }else{
    if (get_bitwidth(__LINE__) == 32) {
      o << "[[" << (int)x1 << ", " << (int)y1 << "]]";
    } else if (get_bitwidth(__LINE__) == 8) {
      o << "[[" << (int)static_cast<signed char>(x1) << ", "
        << (int)static_cast<signed char>(y1) << "]]";
    } else {
      o << "[[" << m_start_1.get_signed_bignum() << ", "
        << m_end_1.get_signed_bignum() << "]]";
    }
  }

  o << "}_";
  
#else
  o << "{[" << m_start_0 << ", " << m_end_0 << "], " << 
    m_start_1 << ", " << m_end_1 << "]}_";
#endif
  o << (int)get_bitwidth(__LINE__);
  }
}

} // namespace domains
} // namespace crab
