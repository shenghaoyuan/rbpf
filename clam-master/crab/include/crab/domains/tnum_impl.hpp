#pragma once

#include <crab/domains/tnum.hpp>
#include <crab/support/debug.hpp>
#include <crab/support/stats.hpp>

#include <boost/optional.hpp>

#define PRINT_WRAPINT_AS_SIGNED

namespace crab {
namespace domains {

template <typename Number>
tnum<Number> tnum<Number>::default_implementation(
    const tnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  } else {
    return tnum<Number>::top();
  }
}

template <typename Number>
tnum<Number>::tnum(wrapint value, wrapint mask,
                                           bool is_bottom)
    : m_value(value), m_mask(mask), m_is_bottom(is_bottom) {
  CRAB_LOG("tnum-imply", crab::outs() << "tnum of (" << value << ", " << mask << ")" <<  "\n";);
  if (value.get_bitwidth() != mask.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in tnum ");
  }
}



//Returns the minimum number of trailing zero bits.
template <typename Number>
uint64_t tnum<Number>::countMinTrailingZeros() const{
  wrapint max = m_value + m_mask;
  return max.countr_zero();
}

// Returns the maximum number of trailing zero bits possible.
template <typename Number>
uint64_t tnum<Number>::countMaxTrailingZeros() const{
  return m_value.countr_zero();
}

// Returns the minimum number of leading zero bits.
template <typename Number>
uint64_t tnum<Number>::countMinLeadingZeros() const{
  wrapint max = m_value + m_mask;
  return max.countl_zero();
}

// Returns the maximum number of leading zero bits possible.
template <typename Number>
uint64_t tnum<Number>::countMaxLeadingZeros() const{
  return m_value.countl_zero();
}

template <typename Number>
tnum<Number> tnum<Number>::Shl(uint64_t k) const {
  
  if (is_bottom())
    return *this;

  // XXX: we need the check is_top before calling get_bitwidth();
  if (is_top())
    return *this;
  wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  return tnum<Number>(m_value << wrapint(k, w), m_mask << wrapint(k, w));
}

template <typename Number>
tnum<Number> tnum<Number>::LShr(uint64_t k) const {
  
  if (is_bottom())
    return *this;

  // XXX: we need the check is_top before cross_signed_limit calls
  // get_bitwidth();
  if (is_top())
    return *this;

  wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  wrapint len(k, w);
  return tnum<Number>(m_value.lshr(len), m_mask.lshr(len));
}

template <typename Number>
tnum<Number> tnum<Number>::AShr(uint64_t k) const {
 
  if (is_bottom())
    return *this;

  // XXX: we need the check is_top before cross_signed_limit calls
  // get_bitwidth();
  if (is_top())
    return *this;

  wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  wrapint len(k, w);
  bool vsig = m_value.msb();
  bool msig = m_mask.msb();
  if(!vsig && !msig) {
    return tnum<Number>(m_value.lshr(len), m_mask.lshr(len));
  } else if (vsig && !msig) {
    return tnum<Number>(m_value.ashr(len), m_mask.lshr(len));
  } else {
    return tnum<Number>(m_value.lshr(len), m_mask.ashr(len));
  }
}

template <typename Number>
tnum<Number>::tnum(wrapint n)
    : m_value(n), m_mask(wrapint(0, n.get_bitwidth())), m_is_bottom(false) {}

template <typename Number>
tnum<Number>::tnum(wrapint value, wrapint mask)
    : m_value(value), m_mask(mask), m_is_bottom(false) {
  if(!(value & mask).is_zero()){
    m_is_bottom = true;
  }
  CRAB_LOG("tnum-imply", crab::outs() << "tnum2" <<  "\n";);
  if (value.get_bitwidth() != mask.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in tnum");
  }
}

template <typename Number>
tnum<Number>::tnum(wrapint min, wrapint max, bitwidth_t width)
    : m_value(wrapint(0, width)), m_mask(wrapint::get_unsigned_max(width)), m_is_bottom(false) {
  wrapint chi = min ^ max;
  uint64_t bits = chi.fls();
  if(bits <= 63){
    wrapint delta = wrapint(1, width) << wrapint(bits -1, width);
    this->m_value = min & (~delta);
    this->m_mask = delta;
    if(max < min){
      this->m_is_bottom = true;
      this->m_value = wrapint::get_unsigned_max(width);
    }
  }
  
}


// To represent top, the particular bitwidth here is irrelevant. We
// just make sure that each bit is masked
template <typename Number>
tnum<Number>::tnum()
    : m_value(wrapint(0, 3)), m_mask(7, 3), m_is_bottom(false) {}

// return top if n does not fit into a wrapint. No runtime errors.
template <typename Number>
tnum<Number>
tnum<Number>::mk_tnum(Number n, wrapint::bitwidth_t width) {
  CRAB_LOG("tnum-imply", crab::outs() << "mk_tnum of " << n << ":\n";);
  if (wrapint::fits_wrapint(n, width)) {
    return tnum<Number>(wrapint(n, width));
  } else {
    CRAB_WARN(n, " does not fit into a wrapint. Returned top tnum");
    return tnum<Number>::top(width);
  }
}

// Return top if lb or ub do not fit into a wrapint. No runtime errors.
template <typename Number>
tnum<Number>
tnum<Number>::mk_tnum(Number lb, Number ub,
                                       wrapint::bitwidth_t width) {
  CRAB_LOG("tnum-imply", crab::outs() << "mk_tnum of " << lb << ub << ":\n";);                                      
  if (!wrapint::fits_wrapint(lb, width)) {
    CRAB_WARN(lb,
              " does not fit into a wrapint. Returned top tnum");
    return tnum<Number>::top();
  } else if (!wrapint::fits_wrapint(ub, width)) {
    CRAB_WARN(ub,
              " does not fit into a wrapint. Returned top tnum");
    return tnum<Number>::top();
  } else {
  /*  auto tnum_from_range = [&] (const wrapint &min, const wrapint &max){
      wrapint chi = min ^ max;
      uint64_t bits = chi.fls();
      if(bits > 63){
        return tnum<Number>::top();
      }
      wrapint delta = wrapint(1, width) << wrapint(bits -1, width);
      return tnum<Number>(min & (~delta), delta, __LINE__);
    };*/
    wrapint lbwrap(lb, width);
    wrapint ubwrap(ub, width);
    bool lbflag = lbwrap.msb();
    bool ubflag = ubwrap.msb();
    if (!(lbflag ^ ubflag)) {
      return tnum_from_range(lbwrap, ubwrap);
    } else {
      tnum<Number> pos = tnum_from_range(wrapint::get_unsigned_min(width), ubwrap);
      tnum<Number> neg = tnum_from_range(lbwrap, wrapint::get_unsigned_max(width));
      return pos | neg;
    }
    
  }
}

template <typename Number>
tnum<Number> tnum<Number>::top() {
  return tnum<Number>(wrapint(0, 3), wrapint(7, 3), false);
}

template <typename Number>
tnum<Number> tnum<Number>::top(bitwidth_t width) {
  return tnum<Number>(wrapint::get_unsigned_min(width), wrapint::get_unsigned_max(width), false);
}

template <typename Number>
tnum<Number> tnum<Number>::bottom() {
  // the wrapint is irrelevant.
  wrapint i(7, 3);
  return tnum<Number>(i, i, true);
}

template <typename Number>
tnum<Number> tnum<Number>::bottom(bitwidth_t width) {
  // the wrapint is irrelevant.
  wrapint umax = wrapint::get_unsigned_max(width);
  return tnum<Number>(umax, umax, true);
}

/*

// return interval [0111...1, 1000....0]
// In the APLAS'12 paper "signed limit" corresponds to "north pole".
template <typename Number>
tnum<Number>
tnum<Number>::signed_limit(wrapint::bitwidth_t b) {
  return tnum<Number>(wrapint::get_signed_max(b),
                                  wrapint::get_signed_min(b));
}

// return interval [1111...1, 0000....0]
// In the APLAS'12 paper "unsigned limit" corresponds to "south pole".
template <typename Number>
tnum<Number>
tnum<Number>::unsigned_limit(wrapint::bitwidth_t b) {
  return tnum<Number>(wrapint::get_unsigned_max(b),
                                  wrapint::get_unsigned_min(b));
}

template <typename Number>
bool tnum<Number>::cross_signed_limit() const {
  return (signed_limit(get_bitwidth(__LINE__)) <= *this);
}

template <typename Number>
bool tnum<Number>::cross_unsigned_limit() const {
  return (unsigned_limit(get_bitwidth(__LINE__)) <= *this);
}

*/

template <typename Number>
wrapint::bitwidth_t tnum<Number>::get_bitwidth(int line) const {
  if (is_bottom()) {
    CRAB_ERROR("get_bitwidth() cannot be called from a bottom element at line ",
               line);
  } else if (is_top()) {
    CRAB_ERROR("get_bitwidth() cannot be called from a top element at line ",
               line);
  } else {
    assert(m_value.get_bitwidth() == m_mask.get_bitwidth());
    return m_value.get_bitwidth();
  }
}

template <typename Number> wrapint tnum<Number>::value() const {
  if (is_top()) {
    CRAB_ERROR("method value() cannot be called if top");
  }
  return m_value;
}

template <typename Number> wrapint tnum<Number>::mask() const {
  if (is_top()) {
    CRAB_ERROR("method mask() cannot be called if top");
  }
  return m_mask;
}

template <typename Number> wrapint tnum<Number>::getSignedMaxValue() const{
  wrapint max = m_value + m_mask;
  if(m_mask.msb()) {
    max.clearBit(m_value.get_bitwidth()-1);
  }
  return max;
}

template <typename Number> wrapint tnum<Number>::getSignedMinValue() const{
  //CRAB_LOG("tnum-imply", crab::outs() << "getSignedMinValue of " << *this << ":\n";);
  
  /*if(m_value.is_signed_min()){
    //CRAB_LOG("tnum-imply", crab::outs() << "min getSignedMinValue of " << *this << " = " << m_value<< ":\n";);
    return m_value;
  }*/
  wrapint min = m_value;
  if(m_mask.msb()) {
    //CRAB_LOG("tnum-imply", crab::outs() << "mask msb is 1 " << ":\n";);
    min.setBit(m_value.get_bitwidth()-1);
  }
  //CRAB_LOG("tnum-imply", crab::outs() << "getSignedMinValue of " << *this << " = " << min<< ":\n";);
  return min;
}

template <typename Number>  tnum<Number> 
tnum<Number>::getZeroCircle() const{
  CRAB_LOG("tnum-imply", crab::outs() << "getZeroCircle of " << *this << ":\n";);
  assert(!is_top() && !is_bottom());

  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  wrapint sign_max = wrapint::get_signed_max(width);
  
  if(m_value.msb()){
    return tnum<Number>(sign_max, sign_max, true);
  }else if(m_mask.msb()){
    return tnum<Number>(m_value, m_mask & sign_max, false);
  }else {
    return *this;
  }
}


template <typename Number>  tnum<Number> 
tnum<Number>::getOneCircle() const{
  CRAB_LOG("tnum-imply", crab::outs() << "getOneCircle of " << *this << ":\n";);
  assert(!is_top() && !is_bottom());

  wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  wrapint sign_max = wrapint::get_signed_max(width);
  wrapint sign_min = wrapint::get_signed_min(width);
  wrapint unsign_max = wrapint::get_unsigned_max(width);
  if(m_value.msb()){
    return *this;
  }else if(m_mask.msb()){
    wrapint value = m_value;
    value.setBit(width-1);
    wrapint mask = m_mask;
    mask.clearBit(width-1);
    return tnum<Number>(value, mask, false);
  }else {
    return tnum<Number>(unsign_max, unsign_max, true);
  }
}


template <typename Number> bool tnum<Number>::is_bottom() const {
  //CRAB_LOG("tnum-imply", crab::outs()<< "(" << m_value << ", " << m_mask << ")" <<  "is bottom?" << ":";);
  wrapint flag = (m_value & m_mask);
  bool res = (m_is_bottom || !flag.is_zero());
  //CRAB_LOG("tnum-imply", crab::outs() <<  res << "\n";);
  return res;
}

template <typename Number> bool tnum<Number>::is_top() const {
  //CRAB_LOG("tnum-imply", crab::outs() << "(" << m_value << ", " << m_mask << ")" << "is top?" << ": ";);
  //wrapint maxspan = wrapint::get_unsigned_max(m_value.get_bitwidth());
  bool flag = (!m_is_bottom && (m_value.is_zero()) && (m_mask.is_unsigned_max()));
  //CRAB_LOG("tnum-imply", crab::outs() << flag << "\n";);
  return flag;
}

template <typename Number> bool tnum<Number>::is_negative() const{
  return (m_value.msb() && !m_mask.msb());
}

template <typename Number> bool tnum<Number>::is_nonnegative() const{
  return (!m_value.msb() && !m_mask.msb());
}

template <typename Number> bool tnum<Number>::is_zero() const{
  return (m_value.is_zero() && m_mask.is_zero());
}

template <typename Number> bool tnum<Number>::is_positive() const{
  return (!m_value.msb() && !m_mask.msb() && (!m_value.is_zero()));
}

template <typename Number>
ikos::interval<Number> tnum<Number>::to_interval() const {
  using interval_t = ikos::interval<Number>;
  
  if (is_bottom()) {
    return interval_t::bottom();
  } else if (is_top()) {
    return interval_t::top();
  } else if (m_mask.msb()){
    wrapint::bitwidth_t b = get_bitwidth(__LINE__);
    wrapint negmax = wrapint::get_signed_min(b) | m_value;
    wrapint posmax = wrapint::get_signed_max(b) & (m_value + m_mask);
    return interval_t(negmax.get_signed_bignum(), posmax.get_signed_bignum());
  } else {
    return interval_t(m_value.get_signed_bignum(), (m_value + m_mask).get_signed_bignum());
  }
}



// need to fix, as from x <= y and y, the result of x may >= y  
// very imprecise, we donot use this in the solver
template <typename Number>
tnum<Number>
tnum<Number>::lower_half_line(bool is_signed) const {
  if (is_top() || is_bottom())
    return *this;

  return tnum<Number>::top();
  
}

// we use this version in the solver to propagate
// make use of x's lb
template <typename Number>
tnum<Number>
tnum<Number>::lower_half_line(const tnum<Number> &x, bool is_signed) const {
  CRAB_LOG("tnum-imply-low", crab::outs() << "lower_half_line of " <<*this << " and " << x << " : \n";);
  if (is_top() || is_bottom()){
    CRAB_LOG("tnum-imply-low", crab::outs() << "lower_half_line of 1"<< "\n";);
    return *this;
  }
  wrapint::bitwidth_t w = get_bitwidth(__LINE__);
/*  auto tnum_from_range = [&] (const wrapint &min, const wrapint &max){
      wrapint chi = min ^ max;
      uint64_t bits = chi.fls();
      if(bits > 63){
        return tnum<Number>::top();
      }
      wrapint delta = wrapint(1, w) << wrapint(bits -1, w);
       CRAB_LOG("tnum-imply", crab::outs() << "lower_half_line of 2"<< "\n";);
      return tnum<Number>(min & (~delta), delta, __LINE__);
  };*/
  wrapint xmin = wrapint(0, w);
  if(x.is_top()){
    if(is_signed){
      xmin = wrapint::get_signed_min(w);
    }else{
      xmin = wrapint::get_unsigned_min(w);
    }
  } else if (x.is_bottom()){
    return tnum<Number>::bottom();
  }else{
    xmin = x.getSignedMinValue();
  }
  
  wrapint max = getSignedMaxValue();
  
  if (is_signed) {
    if(max.get_signed_bignum() < xmin.get_signed_bignum())
      return tnum<Number>::bottom();
    bitwidth_t width = max.get_bitwidth();
    wrapint lbwrap = xmin;
    wrapint ubwrap = max;
    // CRAB_LOG("tnum-imply", crab::outs() << "lower_half_line of 3"<< lbwrap << ubwrap << "\n";);
    bool lbflag = lbwrap.msb();
    bool ubflag = ubwrap.msb();
    if (!(lbflag ^ ubflag)) {
      tnum<Number> res = tnum_from_range(lbwrap, ubwrap);
      CRAB_LOG("tnum-imply-low", crab::outs() << "lower_half_line res = " << res << "\n";);
      return res;
    } else {
      tnum<Number> pos = tnum_from_range(wrapint::get_unsigned_min(width), ubwrap);
      tnum<Number> neg = tnum_from_range(lbwrap, wrapint::get_unsigned_max(width));
      tnum<Number> res = pos | neg;
      CRAB_LOG("tnum-imply-low", crab::outs() << "lower_half_line res = " << res << "\n";);
      return res;
    }
  } else {
    max = m_value + m_mask;
    if (xmin > max) return tnum<Number>::bottom();
    return tnum_from_range(xmin, max);
  }

  
}

template <typename Number>
tnum<Number>
tnum<Number>::tnum_from_range(wrapint min, wrapint max) {
  wrapint::bitwidth_t w = min.get_bitwidth();
  CRAB_LOG("tnum-imply-tnum_from_range", crab::outs() << "min = "<< min << ", max = " << max << "\n";);
  wrapint chi = min ^ max;
  uint64_t bits = chi.fls();
  CRAB_LOG("tnum-imply-tnum_from_range", crab::outs() << "chi = "<< chi << ", bits = " << bits << "\n";);
  if(bits > w-1){
    return tnum<Number>::top();
  }
  wrapint delta =(wrapint(1, w) << wrapint(bits, w)) - wrapint(1, w);
  tnum<Number> unsign_tnum(min & (~delta), delta);
  CRAB_LOG("tnum-imply-tnum_from_range", crab::outs() << "tnum_from_range of "<< min << " and " 
            << max << " is: " << unsign_tnum << "\n";);
  return unsign_tnum;
}

// need to fix, as from x <= y and y, the result of x may >= y  
// very imprecise, we donot use this in the solver
template <typename Number>
tnum<Number>
tnum<Number>::upper_half_line(bool is_signed) const {
  if (is_top() || is_bottom())
    return *this;

  return tnum<Number>::top();
  
}


// we use this version in the solver to propagate
// make use of x's ub
template <typename Number>
tnum<Number>
tnum<Number>::upper_half_line(const tnum<Number> &x, bool is_signed) const {
  CRAB_LOG("tnum-imply-upper", crab::outs() << "upper_half_line of " <<*this << " and " << x << " : \n";);
  if (is_top() || is_bottom())
    return *this;
  wrapint::bitwidth_t w = get_bitwidth(__LINE__);
/*  auto tnum_from_range = [&] (const wrapint &min, const wrapint &max){
      wrapint chi = min ^ max;
      uint64_t bits = chi.fls();
      if(bits > 63){
        return tnum<Number>::top();
      }
      wrapint delta = wrapint(1, w) << wrapint(bits -1, w);
      return tnum<Number>(min & (~delta), delta, __LINE__);
  };*/
  wrapint xmax = wrapint(0, w);
  if(x.is_top()){
    if(is_signed){
      xmax = wrapint::get_signed_max(w);
    }else{
      xmax = wrapint::get_unsigned_max(w);
    }
  }else if(x.is_bottom()){
    return tnum<Number>::bottom();
  }else{
    xmax = x.getSignedMaxValue();
  }
   wrapint min = m_value;
   //CRAB_LOG("tnum-imply", crab::outs() << "upper_half_line of 1"<< "\n";);
  if (is_signed) {
    if(xmax.get_signed_bignum() < min.get_signed_bignum())
      return tnum<Number>::bottom();
    bitwidth_t width = min.get_bitwidth();
    wrapint lbwrap = min;
    wrapint ubwrap = xmax;
    CRAB_LOG("tnum-imply-upper", crab::outs() <<"upper_half_line lbwrap = " 
              << lbwrap << ", ubwrap = " << ubwrap << "\n";);
    bool lbflag = lbwrap.msb();
    bool ubflag = ubwrap.msb();
    if (!(lbflag ^ ubflag)) {
      tnum<Number> res = tnum_from_range(lbwrap, ubwrap);
       CRAB_LOG("tnum-imply-upper", crab::outs() <<"upper_half_line res = " << res << "\n";);
      return res;
    } else {
      tnum<Number> pos = tnum_from_range(wrapint::get_unsigned_min(width), ubwrap);
      tnum<Number> neg = tnum_from_range(lbwrap, wrapint::get_unsigned_max(width));
      tnum<Number> res = pos | neg;
       CRAB_LOG("tnum-imply-upper", crab::outs() <<"upper_half_line res = " <<  res << "\n";);
      return res;
    }
  } else {
    if (xmax < min) return tnum<Number>::bottom();
    return tnum_from_range(min, xmax);
  }

  
}





template <typename Number> bool tnum<Number>::is_singleton() const {
  return (!is_bottom() && !is_top() && m_mask.is_zero());
}

// determine if the unmask bits is equal to x
template <typename Number> bool tnum<Number>::at(wrapint x) const {
  CRAB_LOG("tnum-imply", crab::outs() << x << "at" << *this <<":\n";);
  if (is_bottom()) {
    return false;
  } else if (is_top()) {
    return true;
  } else {
    CRAB_LOG("tnum-imply", crab::outs()  << "at"  <<":\n";);
    bool res = (m_value == ((~m_mask) & x));
    return res;
  }
}

template <typename Number>
bool tnum<Number>::operator<=(
    const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "<=" << x << ":\n";);
  if (x.is_top() || is_bottom()) {
    return true;
  } else if (x.is_bottom() || is_top()) {
    return false;
  } else if (m_value == x.m_value && m_mask == x.m_mask){
    return true;
  } else if (!(m_mask & (~x.m_mask)).is_zero()){ //this[i] unknow but x[i] know
    return false;
  }
  else {
    return ((m_value & (~x.m_mask)) == x.m_value);
  }
}

template <typename Number>
bool tnum<Number>::operator==(
    const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "==" << x << ":\n";);
  return (*this <= x && x <= *this);
}

template <typename Number>
bool tnum<Number>::operator!=(
    const tnum<Number> &x) const {
   CRAB_LOG("tnum-imply", crab::outs() << *this << "!=" << x << ":\n";);
  return !(this->operator==(x));
}

template <typename Number>
tnum<Number>
tnum<Number>::operator|(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "U" << x << ":\n";);
  if (*this <= x) {
    return x;
  } else if (x <= *this) {
    return *this;
  } else {
    wrapint mu = m_mask | x.m_mask;
    wrapint this_know = m_value & (~mu);
    wrapint x_know = x.m_value & (~mu);
    wrapint disagree = this_know ^ x_know;
    return tnum<Number>(this_know & x_know, mu | disagree);
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::operator&(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "M" << x << ":\n";);
  if (*this <= x) {
    return *this;
  } else if (x <= *this) {
    return x;}
  //crab::outs() << "this = " <<*this << "x=" <<x<<"\n";
  
  wrapint mu1 = m_mask & x.m_mask;
  wrapint mu2 = m_mask | x.m_mask;
  wrapint this_known_v = m_value & (~mu2);
  wrapint x_known_v = x.m_value & (~mu2);
  wrapint disagree = this_known_v ^ x_known_v;
  //crab::outs() << "operator&2" << "\n";
  if (!disagree.is_zero()) 
    return tnum<Number>::bottom();
  return tnum<Number>((m_value | x.m_value) & (~mu1), mu1);
}

/* height of lattice is limited width */

template <typename Number>
tnum<Number>
tnum<Number>::operator||(const tnum<Number> &x) const {
  // make all least bits unknown.
  // crab::outs() << "tnum-imply-widen "<< *this << " || " << x << " : \n";
  if (is_bottom()) {
    return x;
  } else if (x.is_bottom()) {
    return *this;
  } else {
    uint64_t tr_zero = m_mask.countr_zero();
    uint64_t ld_zero = m_mask.countl_zero();
    uint64_t xtr_zero = x.m_mask.countr_zero();
    uint64_t xld_zero = x.m_mask.countl_zero();
    if( (tr_zero == xtr_zero) && (ld_zero == (xld_zero+1)) && tr_zero != 0){
      wrapint common_value = m_value & x.m_value;
      bitwidth_t w = common_value.get_bitwidth(); 
      common_value.clearHighBits(w-tr_zero);
      wrapint mask = wrapint::get_unsigned_max(w);
      mask.clearLowBits(tr_zero);
      tnum<Number> res  = tnum<Number>(common_value, mask);
      //crab::outs() << "tnum-imply-widen res = "<< res << "  \n";
      return res;

    }else{
      crab::outs() << "tnum-imply-widen "<< *this << " || " << x << " : \n";
      crab::outs() << "tnum-imply-widen, tr_zero > xtr_zero, use join \n";
      return *this | x;
    }
    /*tnum<Number> jr = *this | x;
    wrapint max = jr.m_value + jr.m_mask;
    uint64_t leadingzero = max.countl_zero();
    bitwidth_t w = max.get_bitwidth(); 
    wrapint unsignMax = wrapint::get_unsigned_max(w);
    wrapint unsignMin = wrapint::get_unsigned_min(w);
    unsignMax.clearHighBits(leadingzero);
    wrapint resValue = wrapint::get_unsigned_min(w);
    tnum<Number> common = tnum<Number>(common_value, unsignMin);
    
    //crab::outs() << "tnum-imply-widen , resValue = "<< resValue << ",unsignMax = " << unsignMax << "  \n";
    tnum<Number> res  = tnum<Number>(resValue, unsignMax);
     //crab::outs() << "tnum-imply-widen res = "<< res << "  \n";
    
    /*if(!common_value.is_zero()){
      res = res & common;
    }
    wrapint different = ~common_value;
    tnum<Number> res  = tnum<Number>(common_value, different);
    crab::outs() << "tnum-imply-widen res = "<< res << "  \n";
    return res;*/
  }
}


// TODO: factorize code with operator||
template <typename Number>
tnum<Number> tnum<Number>::widening_thresholds(
    const tnum<Number> &x, const thresholds<Number> &ts) const {
      crab::outs() << "widening_thresholds" << "\n";
    return (*this | x);
}



template <typename Number>
tnum<Number>
tnum<Number>::operator&&(const tnum<Number> &x) const {
  // TODO: for now we call the meet operator.
  crab::outs() << "operator&&" << "\n";
  return (*this & x);
}


template <typename Number>
tnum<Number>
tnum<Number>::operator+(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "+" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  } else if (is_top() || x.is_top()) {
    return tnum<Number>::top();
  } else {
    wrapint sm = m_mask + x.m_mask;
    wrapint sv = m_value + x.m_value;
    wrapint sigma = sm + sv;
    wrapint chi = sigma ^ sv;
    wrapint mu = chi | m_mask | x.m_mask;
    tnum<Number> res(sv & (~mu), mu);
    CRAB_LOG("tnum-imply", crab::outs() << *this << "+" << x << " = " << res << "\n";);
    return res;
  }
}

template <typename Number>
tnum<Number> &
tnum<Number>::operator+=(const tnum<Number> &x) {
  //crab::outs() << "operator+=" << "\n";
  return this->operator=(this->operator+(x));
}

template <typename Number>
tnum<Number> tnum<Number>::operator-() const {
  CRAB_LOG("tnum-imply", crab::outs() << "-" << *this << ":\n";);
  if (is_bottom()) {
    return tnum<Number>::bottom();
  } else if (is_top()) {
    return tnum<Number>::top();
  } else {
    wrapint zero(0, get_bitwidth(__LINE__)); 
    tnum<Number> res(zero);
    res = res - *this;
    return res;
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::operator-(const tnum<Number> &x) const {
 CRAB_LOG("tnum-imply", crab::outs() << *this << " - " << x << ":\n";);
  if (is_bottom()|| x.is_bottom()) {
    return tnum<Number>::bottom();
  } else if (is_top()|| x.is_top()) {
    return tnum<Number>::top();
  } else {
    wrapint dv = m_value - x.m_value;
    wrapint alpha = dv + m_mask;
    wrapint beta = dv - x.m_mask;
    wrapint chi = alpha ^ beta;
    wrapint mu = chi | m_mask | x.m_mask;
    tnum<Number> res(dv & (~mu), mu);
    CRAB_LOG("tnum-imply", crab::outs() << *this << " - " << x << " = " << res << "\n";);
    return res;
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::operator~() const {
  CRAB_LOG("tnum-imply", crab::outs() << "~" << *this << ":\n";);
  if (is_bottom()) {
    return tnum<Number>::bottom();
  } else if (is_top()) {
    return tnum<Number>::top();
  } else {
    return tnum<Number>(~(m_value ^ m_mask), m_mask);
  }
}

template <typename Number>
tnum<Number> &
tnum<Number>::operator-=(const tnum<Number> &x) {
  crab::outs() << "operator-=" << "\n";
  return this->operator=(this->operator-(x));
}

template <typename Number>
tnum<Number>
tnum<Number>::operator*(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "*" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return tnum<Number>::top();
  } else {
    wrapint::bitwidth_t w = get_bitwidth(__LINE__);
    wrapint zero(0, w);
    wrapint one(1, w);
    wrapint acc_v = m_value * x.m_value;
    tnum<Number> acc_m(zero, zero);
    tnum<Number> this_tmp = *this;
    tnum<Number> x_tmp = x;
    while((!this_tmp.m_value.is_zero()) || (!this_tmp.m_mask.is_zero())){
      if (!(this_tmp.m_value & one).is_zero())
        acc_m = acc_m + tnum<Number>(zero, x_tmp.m_mask);
      else if (!(this_tmp.m_mask & one).is_zero())
        acc_m = acc_m + tnum<Number>(zero, x_tmp.m_value | x_tmp.m_mask);
      this_tmp = this_tmp.LShr(1);
      x_tmp = x_tmp.Shl(1);
    }
    tnum<Number> res_tmp(acc_v, zero);
    tnum<Number> res = res_tmp + acc_m;
    CRAB_LOG("tnum-imply", crab::outs() << *this << "*" << x << " = " << res <<":\n";);
    return res;
  }
}

template <typename Number>
tnum<Number> &
tnum<Number>::operator*=(const tnum<Number> &x) {
  crab::outs() << "operator*=" << "\n";
  return this->operator=(this->operator*(x));
}

//no use, because its not exact division 
template <typename Number>
tnum<Number>
tnum<Number>::divComputeLowBit(tnum<Number> Known, 
  const tnum<Number> &LHS, const tnum<Number> &RHS){
  //crab::outs() << "divComputeLowBit" << "\n";
  // If LHS is Odd, the result is Odd no matter what.
  // Odd / Odd -> Odd
  CRAB_LOG("tnum-imply-divComputeLowBit", crab::outs() << 
    "divComputeLowBit begin with Known = " <<  Known << "\n";);
  if(LHS.m_value.is_odd() && LHS.m_mask.is_odd()) {
    Known.m_value.setBit(0);
    Known.m_mask.clearBit(0);
  }
  //crab::outs() << "Known1=" << Known << "\n";
  int MinTZ = (int)LHS.countMinTrailingZeros() - (int)RHS.countMaxTrailingZeros();
  int MaxTZ = (int)LHS.countMaxTrailingZeros() - (int)RHS.countMinTrailingZeros();
  CRAB_LOG("tnum-imply-divComputeLowBit", crab::outs() << 
    "divComputeLowBit compute with MinTZ = " <<  MinTZ << ", MaxTZ = " << MaxTZ << "\n";);
  //crab::outs() << "divComputeLowBit1" << "\n";
  //crab::outs() << "Known2=" << Known << "\n";
  if (MinTZ >= 0) {
    // Result has at least MinTZ trailing zeros.
    Known.m_value.clearLowBits(MinTZ);
    Known.m_mask.clearLowBits(MinTZ);
    CRAB_LOG("tnum-imply-divComputeLowBit", crab::outs() << 
      "divComputeLowBit after trailing zero, Known = " <<  Known << "\n";);

    if(MinTZ == MaxTZ) {
      // Result has exactly MinTZ trailing ones.
      Known.m_value.setBit(MinTZ);
      Known.m_mask.clearBit(MinTZ);
    }
    //crab::outs() << "Known4=" << Known << "\n";
  }
  //crab::outs() << "divComputeLowBit2" << "\n";
  wrapint::bitwidth_t  w = LHS.get_bitwidth(__LINE__);
  if(Known.is_bottom()) 
    return top(w);
  //crab::outs() << "divComputeLowBit3" << "\n";
  CRAB_LOG("tnum-imply-divComputeLowBit", crab::outs() << "divComputeLowBit of " 
              << LHS << " and " << RHS << " = " << Known << "\n";);
  return Known;
}

template <typename Number>
tnum<Number>
tnum<Number>::remGetLowBits(const tnum<Number> &LHS, const tnum<Number> &RHS) const {
   CRAB_LOG("tnum-imply-remGetLowBits", crab::outs() << "remGetLowBits of " 
              << LHS << " and " << RHS << "\n";);
  wrapint::bitwidth_t  w = LHS.get_bitwidth(__LINE__);
  if(!RHS.is_zero() && RHS.m_value.is_even() && RHS.m_mask.is_even()) {
    uint64_t qzero = RHS.countMinTrailingZeros();
    if(qzero = 0) return top(w); // 
    wrapint mask((((uint64_t)1 << (uint64_t)(qzero - 1)) - 1), w);
    wrapint res_value = LHS.m_value & mask;
    wrapint res_mask = LHS.m_mask & mask;
    tnum<Number> res(res_value, res_mask);
    CRAB_LOG("tnum-imply-remGetLowBits", crab::outs() << "remGetLowBits of " << LHS 
              << " and " << RHS << " = " << res << "\n";);
    return res;
  }
  return top(w);
}

template <typename Number>
tnum<Number>
tnum<Number>::signed_div(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this <<"signed_div" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }
  wrapint::bitwidth_t w =  tnum<Number>::get_bitwidth(__LINE__);
  if(m_mask.is_zero() && x.m_mask.is_zero()){ // certain sdiv certain
    return tnum<Number>(m_value.sdiv(x.m_value), wrapint::get_unsigned_min(w));
  } 

  if(!m_mask.msb() && !m_value.msb() && !x.m_mask.msb() && !x.m_value.msb()) {
    return UDiv(x);
  }
  tnum<Number> Res = tnum<Number>::top(w);
  wrapint tmp(0, w);

  /* determine leading bits */
  if(is_negative() && x.is_negative()) {
    // Result is non-negative

    //when INT_MIN/-1, return top
    if(m_value.is_signed_min() && x.m_value.is_unsigned_max() && x.m_mask.is_zero()){
      return tnum<Number>::top(w);
    }

    wrapint Denom = x.getSignedMaxValue();
    
    wrapint Num = getSignedMinValue();
    //when INT_MIN/-1,only determin result is pos
    CRAB_LOG("tnum-imply-s-div", crab::outs() << "Denom = " << Denom << ", Num = "<< Num << "\n";);
    if(!(Num.is_signed_min() && Denom.is_signed_max())) {
      tmp = Num.sdiv(Denom);
    }else{
      tmp = wrapint::get_signed_max(w);
    }
  } else if(is_negative() && x.is_positive()) {
    // Result is negative if -LHS u>= RHS
    if((-getSignedMaxValue()) >= x.getSignedMaxValue()){
      wrapint Denom = x.getSignedMinValue();
      wrapint Num = getSignedMinValue();
      tmp = Num.sdiv(Denom);
    }
    
  } else if (is_positive() && x.is_negative()) {
    // Result is negative if LHS u>= -RHS
    if(getSignedMinValue() >= (-x.getSignedMinValue())){
      wrapint Denom = x.getSignedMaxValue();
      wrapint Num = getSignedMaxValue();
      tmp = Num.sdiv(Denom);
      //CRAB_LOG("tnum-imply-s-div", crab::outs() << "Denom = " << Denom << ", NUm = " << Num << "\n";);
    }
    
  }
  CRAB_LOG("tnum-imply-s-div", crab::outs() << "tmp = " << tmp << "\n";);
  if(!tmp.is_zero()) {
    if(!tmp.msb()) {
      uint64_t LeadZero = tmp.countl_zero();
      Res.m_value.clearHighBits(LeadZero);
      Res.m_mask.clearHighBits(LeadZero);
    } else {
      uint64_t LeadOne = (~tmp).countl_zero();
      Res.m_value.setHighBits(LeadOne);
      Res.m_mask.clearHighBits(LeadOne);
    }
  }
  CRAB_LOG("tnum-imply-s-div", crab::outs() << "after high Res = " << Res << "\n";);
  /* determine low bits */ //no use because analysis is not exact
  //Res = divComputeLowBit(Res, *this, x);
  CRAB_LOG("tnum-imply-s-div", crab::outs() << *this << " SDiv " << x << " = " << Res << "\n";);
  return Res;
}

/** division and remainder operations **/
template <typename Number>
tnum<Number>
tnum<Number>::SDiv(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply-sdiv", crab::outs() << *this << " SDiv " << x << ":\n";);
  CRAB_LOG("tnum-imply", crab::outs() << *this << " SDiv " << x << ":\n";);
  
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }
  
  if (is_top() || x.is_top()) {
    return tnum<Number>::top();
  } 

  wrapint::bitwidth_t w =  tnum<Number>::get_bitwidth(__LINE__);
  if(x.m_value.is_zero()){
    return tnum<Number>::top();
  }else if(m_mask.is_zero() && x.m_mask.is_zero()){ // certain sdiv certain
    return tnum<Number>(m_value.sdiv(x.m_value), wrapint::get_unsigned_min(w));
  } 
  else { 
    
    tnum_t t0 = getZeroCircle();
    tnum_t t1 = getOneCircle();
    tnum_t x0 = x.getZeroCircle();
    tnum_t x1 = x.getOneCircle();

    tnum_t res00 = t0.signed_div(x0);
    tnum_t res01 = t0.signed_div(x1);
    tnum_t res10 = t1.signed_div(x0);
    tnum_t res11 = t1.signed_div(x1);

    CRAB_LOG("tnum-imply-sdiv", crab::outs() << "res01 = " << res01 << "\n";);

    tnum_t res = res00 | res01 | res10 | res11;
    CRAB_LOG("tnum-imply-sdiv", crab::outs() << *this << " SDiv " << x << " = " << res << "\n";);
    return res;
  }
}


template <typename Number>
tnum<Number>
tnum<Number>::UDiv(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "UDiv" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return tnum<Number>::top();
  }

  wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  bool flag = x.m_value.is_zero();


  if(flag){
    CRAB_LOG("tnum-udiv, 0 at", crab::outs() << x << "\n";);
    return tnum<Number>::top();
  } else {
    tnum<Number> Res = tnum<Number>::top(w);
    wrapint MaxRes = flag?(m_value + m_mask) : (m_value + m_mask).udiv(x.m_value);
    // determine leading bits
    uint64_t LeadZ = MaxRes.countl_zero();
    CRAB_LOG("tnum-udiv", crab::outs() << "MaxRes = "<< MaxRes << ", LeadZ = "<< LeadZ << "\n";);
    Res.m_value.clearHighBits(LeadZ);
    Res.m_mask.clearHighBits(LeadZ);
    CRAB_LOG("tnum-udiv", crab::outs() << "determine leading bits, Res = "<< Res << "\n";);
    if (LeadZ == w ) {
      CRAB_LOG("tnum-udiv", crab::outs() << "Res = "<< Res << "\n";);
      return Res;
    }
      
    // determine low bits
    CRAB_LOG("tnum-udiv", crab::outs() << "determine low bits" << "\n";);
    Res = divComputeLowBit(Res, *this, x);
    CRAB_LOG("tnum-udiv", crab::outs() << "determine low bits, Res = "<< Res << "\n";);
    return Res;
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::SRem(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "SRem" << x << ":\n";);
  
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }else if(is_top() || x.is_top()) {
    return tnum<Number>::top();
  } 
  
    wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  wrapint zero(0, w);  
  if(x.is_singleton() && is_singleton()){  // 
    tnum<Number> resSingle(m_value.srem(x.m_value), zero, false);
    //crab::outs() << *this << "SRem1" << x << " = " << resSingle << "\n";
    return resSingle;
  }


  if(x.m_value.is_zero()) {
    CRAB_LOG("tnum-srem, 0 at", crab::outs() << x << "\n";);
    return tnum<Number>::top(w);
  }
  else {
    // determine low bits
    tnum<Number> res = remGetLowBits(*this, x);
    //determin high bits
    // for x is constant and is powerof2 >0
    if(x.m_mask.is_zero() && !x.m_value.msb() &&
      ((x.m_value.countr_zero() + x.m_value.countl_zero() +1) == x.get_bitwidth(__LINE__))) {
        wrapint LowBits = x.m_value - wrapint(1, x.m_value.get_bitwidth());    
        // If the first operand is non-negative or has all low bits zero, then
        // the upper bits are all zero.
        if(is_nonnegative() || (x.m_value.countr_zero() <= countMinTrailingZeros())) {
          res.m_value = LowBits & res.m_value;
          res.m_mask = LowBits & res.m_mask;
        }
        // If the first operand is negative and not all low bits are zero, then
        // the upper bits are all one.
        if(is_negative() && !(m_value & LowBits).is_zero()){
          res.m_mask = LowBits & res.m_mask;
          res.m_value = (~LowBits) | res.m_value;
        }
        return res;
    }

    // The sign bit is the LHS's sign bit, except when the result of the
    // remainder is zero. The magnitude of the result should be less than or
    // equal to the magnitude of the LHS. Therefore any leading zeros that exist
    // in the left hand side must also exist in the result.
    uint64_t leadingz = countMinLeadingZeros();
    res.m_value.clearHighBits(leadingz);
    res.m_mask.clearHighBits(leadingz);
    return res;
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::URem(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "URem" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  } else if(is_top() || x.is_top()) {
    return tnum<Number>::top();
  } else if(x.m_value.is_zero()) {
    CRAB_LOG("tnum-urem, 0 at", crab::outs() << x << "\n";);
    return tnum<Number>::top();
  }
  else {
    // determine low bits
    tnum<Number> res = remGetLowBits(*this, x);
    //determin high bits
    // for x is constant and is powerof2
    if(x.m_mask.is_zero() &&  !x.m_value.msb() &&
      ((x.m_value.countr_zero() + x.m_value.countl_zero() +1) == x.get_bitwidth(__LINE__))) {
        res.m_value = (x.m_value - wrapint(1, x.m_value.get_bitwidth())) & res.m_value;
        res.m_mask = (x.m_value - wrapint(1, x.m_value.get_bitwidth())) & res.m_mask;
        return res;
    }

    // Since the result is less than or equal to either operand, any leading
    // zero bits in either operand must also exist in the result.
    uint64_t leadingz = std::max(countMinLeadingZeros(), x.countMinLeadingZeros());
    res.m_value.clearHighBits(leadingz);
    res.m_mask.clearHighBits(leadingz);
    return res;
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::ZExt(unsigned bits_to_add) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "ZExt" << bits_to_add << ":\n";);
  if (is_bottom() ) {
    return tnum<Number>::bottom();
  }
  return tnum<Number>(m_value.zext(bits_to_add), m_mask.zext(bits_to_add));
}

template <typename Number>
tnum<Number>
tnum<Number>::SExt(unsigned bits_to_add) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "SExt" << bits_to_add << ":\n";);
  if (is_bottom() ) {
    return tnum<Number>::bottom();
  }
  return tnum<Number>(m_value.sext(bits_to_add), m_mask.sext(bits_to_add));
}

template <typename Number>
tnum<Number>
tnum<Number>::Trunc(unsigned bits_to_keep) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "Trunc" << bits_to_keep << ":\n";);
  if (is_bottom() ) {
    return tnum<Number>::bottom();
  }
  return tnum<Number>(m_value.keep_lower(bits_to_keep), 
    m_mask.keep_lower(bits_to_keep));
}

/// Shl, LShr, and AShr shifts are treated as unsigned numbers
template <typename Number>
tnum<Number>
tnum<Number>::Shl(const tnum_t &x) const {// 
  //crab::outs() << *this << "Shl" << x << ":\n";
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }else if(is_top() || x.is_top()){
    return tnum<Number>::top();
  }

  // only if shift is constant
  if (x.is_singleton()) {
    return Shl(x.value().get_uint64_t());
  } else {
    wrapint::bitwidth_t w = get_bitwidth(__LINE__);
    tnum<Number> Res = tnum<Number>::top();
    // Fast path for a common case when LHS is completely unknown
    uint64_t MinShiftAmount = x.m_value.get_uint64_t();
    if(m_mask.is_unsigned_max()) {
      Res.m_value = Res.m_value << wrapint(MinShiftAmount, w);
      Res.m_mask = Res.m_mask << wrapint(MinShiftAmount, w);
      return Res;
    }
    // Determine maximum shift amount
    uint64_t MaxValue = (x.m_value + x.m_mask).get_uint64_t();

    uint64_t len = (m_value|m_mask).countl_zero();
     tnum<Number> maxRes = tnum<Number>::top(w);
     maxRes.m_mask.clearHighBits(len - MaxValue);

    uint64_t MaxShiftAmount = MaxValue>w? w : MaxValue;
    // Fast path for common case where the shift amount is unknown.
    if(MinShiftAmount==0 && MaxShiftAmount == w) {
      uint64_t mintrailingzeros =countMinTrailingZeros();
      Res.m_value.clearLowBits(mintrailingzeros);
      Res.m_mask.clearLowBits(mintrailingzeros);
      return Res;
    }
    // Find the common bits from all possible shifts
    wrapint::bitwidth_t xw = x.get_bitwidth(__LINE__);
    Res.m_mask = wrapint::get_unsigned_max(w);
    Res.m_value = wrapint::get_unsigned_max(w);
    Res.m_is_bottom = true;
    uint64_t JoinCount = 0;
    for(uint64_t i = MinShiftAmount; i <= MaxShiftAmount; i++) {
      if(x.m_value == ((~x.m_mask) & wrapint(i, xw))) {
        continue;
      }
      JoinCount ++;
      Res = Res | (Shl((uint64_t)i));
      if(JoinCount>8 || Res.is_top()) return tnum<Number>::top();
    }
    if (Res.is_bottom())
      return tnum<Number>::top();
    else
      return  Res; 
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::LShr(const tnum_t &x) const {// 
  //crab::outs() << *this << "LShr" << x << ":\n";
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }else if(is_top() || x.is_top()){
    return tnum<Number>::top();
  }

  // only if shift is constant
  if (x.is_singleton()) {
    return LShr(x.value().get_uint64_t());
  } else {
   wrapint::bitwidth_t w = get_bitwidth(__LINE__);
    tnum<Number> Res = tnum<Number>::top();

    // Fast path for a common case when LHS is completely unknown
    uint64_t MinShiftAmount = x.m_value.get_uint64_t();
 
    // Determine maximum shift amount
    uint64_t MaxValue = (x.m_value + x.m_mask).get_uint64_t();
    uint64_t MaxShiftAmount = MaxValue>w? w : MaxValue;

    
    uint64_t len = (m_value).countl_zero();
     tnum<Number> maxRes = tnum<Number>::top(w);
     //crab::outs() << "LSHR len =" << len << ", other =" <<(len + x.m_value.get_uint64_t()) << "\n";
    if((len + x.m_value.get_uint64_t()) >= w)
      return tnum<Number>(wrapint(0, w), wrapint(0, w));
    else
      maxRes.m_mask.clearHighBits(len + x.m_value.get_uint64_t());


    // Find the common bits from all possible shifts
    wrapint::bitwidth_t xw = x.get_bitwidth(__LINE__);
    Res.m_mask = wrapint::get_unsigned_max(w);
    Res.m_value = wrapint::get_unsigned_max(w);
    Res.m_is_bottom = true;
    uint64_t JoinCount = 0;
    for(uint64_t i = MinShiftAmount; i <= MaxShiftAmount; i++) {
      /*if(x.m_value == ((~x.m_mask) & wrapint(i, xw))) {
        continue;
      }*/
      Res = Res | (LShr((uint64_t)i));
      if(JoinCount>6 || Res.is_top()) return maxRes;
    }
    //crab::outs() << "for " << *this << "LShr" << x << "Res = " << Res << "\n";
    if (Res.is_bottom()){
      //return tnum<Number>::top(); 
      return maxRes;
    }else
      return  Res; 
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::AShr(const tnum_t &x) const {// 
  //crab::outs() << *this << "AShr" << x << ":\n";
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }else if(is_top() || x.is_top()){
    return tnum<Number>::top();
  }

  // only if shift is constant
  if (x.is_singleton()) {
    return AShr(x.value().get_uint64_t());
  } else {
    wrapint::bitwidth_t w = get_bitwidth(__LINE__);
    tnum<Number> Res = tnum<Number>::top();
    // Fast path for a common case when LHS is completely unknown
    uint64_t MinShiftAmount = x.m_value.get_uint64_t();
 
    // Determine maximum shift amount
    uint64_t MaxValue = (x.m_value + x.m_mask).get_uint64_t();
    uint64_t MaxShiftAmount = MaxValue >w? w : MaxValue;

    uint64_t len = (m_value).countl_zero();
    tnum<Number> maxRes = tnum<Number>::top(w);
    if(len >= w) 
      return tnum<Number>(wrapint(0, w), wrapint(0, w));
    else
      maxRes.m_mask.clearHighBits(len);

    // Find the common bits from all possible shifts
    wrapint::bitwidth_t xw = x.get_bitwidth(__LINE__);
    Res.m_mask = wrapint::get_unsigned_max(w);
    Res.m_value = wrapint::get_unsigned_max(w);
    Res.m_is_bottom = true;
    uint64_t JoinCount = 0;
    for(uint64_t i = MinShiftAmount; i <= MaxShiftAmount; i++) {
      /*if(x.m_value == ((~x.m_mask) & wrapint(i, xw))) {
        continue;
      }*/
      Res = Res | (AShr((uint64_t)i));
      if(JoinCount>8 || Res.is_top()) return maxRes;
    }
    if (Res.is_bottom()){
      //return tnum<Number>::top(); 
      return maxRes;
    }else
      return  Res; 
  }
}

template <typename Number>
tnum<Number>
tnum<Number>::And(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "And" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }
  if(is_top() || x.is_top()){
    return tnum<Number>::top();
  }
  wrapint alpha = m_value | m_mask;
  wrapint beta = x.m_value | x.m_mask;
  wrapint v = m_value & x.m_value;
  return tnum<Number>(v, alpha & beta & (~v));
}

template <typename Number>
tnum<Number>
tnum<Number>::Or(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "Or" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }
  if(is_top() || x.is_top()){
    return tnum<Number>::top();
  }
  crab::outs() << "Or" << "\n";
  wrapint v = m_value | x.m_value;
  wrapint mu = m_mask | x.m_mask;
  return tnum<Number>(v, mu & (~v));
}

template <typename Number>
tnum<Number>
tnum<Number>::Xor(const tnum<Number> &x) const {
  CRAB_LOG("tnum-imply", crab::outs() << *this << "Xor" << x << ":\n";);
  if (is_bottom() || x.is_bottom()) {
    return tnum<Number>::bottom();
  }
  if(is_top() || x.is_top()){
    return tnum<Number>::top();
  }
  crab::outs() << "Xor" << "\n";
  wrapint v = m_value ^ x.m_value;
  wrapint mu = m_mask | x.m_mask;
  return tnum<Number>(v & (~mu), mu);
}

template <typename Number>
void tnum<Number>::write(crab::crab_os &o) const {
  if (is_bottom()) {
    o << "_|_";
  } else if (is_top()) {
    o << "top";
  } else {
#ifdef PRINT_WRAPINT_AS_SIGNED
    // print the wrapints as a signed number (easier to read)
    uint64_t x = m_value.get_uint64_t();
    uint64_t y = m_mask.get_uint64_t();
    if (get_bitwidth(__LINE__) == 32) {
      o << "[(" << (int)x << ", " << (int)y << ")]_";
    } else if (get_bitwidth(__LINE__) == 8) {
      o << "[(" << (int)static_cast<signed char>(x) << ", "
        << (int)static_cast<signed char>(y) << ")]_";
    } else {
      o << "[(" << m_value.get_signed_bignum() << ", "
        << m_mask.get_signed_bignum() << ")]_";
    }
#else
    o << "[(" << m_value << ", " << m_mask << ")]_";
#endif
    o << (int)get_bitwidth(__LINE__);
  }
}

} // namespace domains
} // namespace crab
