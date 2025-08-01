#pragma once

#include <crab/domains/stnum.hpp>
#include <crab/support/debug.hpp>
#include <crab/support/stats.hpp>

#include <boost/optional.hpp>

#define PRINT_WRAPINT_AS_SIGNED

namespace crab {
namespace domains {

template <typename Number>
stnum<Number> stnum<Number>::default_implementation(
    const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  } else {
    return stnum<Number>::top();
  }
}


template <typename Number>
stnum<Number>::stnum(tnum_t tnum0, tnum_t tnum1, 
    bool is_bottom0, bool is_bottom1)
    : tnum_0(tnum0), tnum_1(tnum1), m_is_bottom_0(is_bottom0), m_is_bottom_1(is_bottom1) {
  
  if (!tnum0.is_bottom() && !tnum1.is_bottom() 
      &&(tnum0.get_bitwidth(__LINE__) != tnum1.get_bitwidth(__LINE__))) {
    CRAB_ERROR("inconsistent bitwidths in stnum");
  }
}

template <typename Number>
stnum<Number>::stnum(wrapint value_0, wrapint mask_0, 
        wrapint value_1, wrapint mask_1, bool is_bottom_0, bool is_bottom_1) {
        
  tnum_t tnum0(value_0, mask_0, is_bottom_0);
  tnum_t tnum1(value_1, mask_1, is_bottom_1);
  this->tnum_0 = tnum0;
  this->tnum_1 = tnum1;
  this->m_is_bottom_0 = is_bottom_0;
  this->m_is_bottom_1 = is_bottom_1;
  if (value_0.get_bitwidth() != value_1.get_bitwidth()) {
    CRAB_ERROR("inconsistent bitwidths in stnum, for constraction_1");
  }
}

template <typename Number>
stnum<Number>::stnum(tnum_t t0, tnum_t t1)
    : tnum_0(t0), tnum_1(t1), m_is_bottom_0(false), m_is_bottom_1(false) {
 
  bool bottom0 = t0.is_bottom();
  bool bottom1 = t1.is_bottom();
  if(bottom0) this->m_is_bottom_0 = true;
  if(bottom1) this->m_is_bottom_1 = true;
  if (!bottom0 && !bottom1
      && t0.get_bitwidth(__LINE__) != t1.get_bitwidth(__LINE__)) {
    CRAB_ERROR("inconsistent bitwidths in stnum , for constraction_2");
  }
}

template <typename Number>
stnum<Number> stnum<Number>::Shl(uint64_t k) const {
  tnum_t t0 = tnum_0.Shl(k);
  tnum_t t1 = tnum_1.Shl(k);
  return stnum<Number>::normalize(t0, t1);
}

template <typename Number>
stnum<Number> stnum<Number>::LShr(uint64_t k) const {
  tnum_t t0 = tnum_0.LShr(k);
  tnum_t t1 = tnum_1.LShr(k);
  return stnum<Number>::normalize(t0, t1);
}

template <typename Number>
stnum<Number> stnum<Number>::AShr(uint64_t k) const {
  tnum_t t0 = tnum_0.AShr(k);
  tnum_t t1 = tnum_1.AShr(k);
  return stnum<Number>::normalize(t0, t1);
}
/*
template <typename Number>
stnum<Number>::stnum(wrapint n)
    : m_value(n), m_mask(wrapint(0, n.get_bitwidth())), m_is_bottom(false) {}

template <typename Number>
stnum<Number>::stnum(wrapint value, wrapint mask)
    : m_value(value), m_mask(mask), m_is_bottom(false) {

}
*/

// To represent top, the particular bitwidth here is irrelevant. We
// just make sure that each bit is masked
template <typename Number>
stnum<Number>::stnum()
    : tnum_0(tnum<Number>(wrapint(0, 3), wrapint(3, 3), false)), tnum_1(tnum<Number>(wrapint(4, 3), wrapint(3, 3), false)), 
              m_is_bottom_0(false), m_is_bottom_1(false){}

// return top if n does not fit into a wrapint. No runtime errors.
template <typename Number>
stnum<Number>
stnum<Number>::mk_stnum(Number n, wrapint::bitwidth_t width) {
  if (wrapint::fits_wrapint(n, width)) {
    wrapint nw = wrapint(n, width);
    stnum_t bottom = stnum<Number>::bottom(width);
    if(nw.msb()) {
      tnum_t t1(nw);
      return stnum<Number>(bottom.tnum_0, t1, true, false);
    } else {
      tnum_t t0(nw);
      return stnum<Number>(t0, bottom.tnum_1, false, true);
    }
  } else {
    CRAB_WARN(n, " does not fit into a wrapint. Returned top stnum");
    return stnum<Number>::top(width);
  }
}

// Return top if lb or ub do not fit into a wrapint. No runtime errors.
template <typename Number>
stnum<Number>
stnum<Number>::mk_stnum(Number lb, Number ub,
                                       wrapint::bitwidth_t width) {
  CRAB_LOG("stnum-imply", crab::outs() << "mk_stnum of " << lb << " and " << ub << " : \n";);
 if (!wrapint::fits_wrapint(lb, width)) {
    CRAB_WARN(lb,
              " does not fit into a wrapint. Returned top stnum");
    return stnum<Number>::top();
  } else if (!wrapint::fits_wrapint(ub, width)) {
    CRAB_WARN(ub,
              " does not fit into a wrapint. Returned top stnum");
    return stnum<Number>::top();
  } else {
    wrapint lbwrap(lb, width);
    wrapint ubwrap(ub, width);
    bool lbflag = lbwrap.msb();
    bool ubflag = ubwrap.msb();
    if (!(lbflag ^ ubflag)) {
      tnum_t t = stnum<Number>::tnum_from_range_s(lbwrap, ubwrap);
      tnum_t t0(wrapint::get_signed_max(width),  wrapint::get_signed_max(width), true);
      tnum_t t1(wrapint::get_unsigned_max(width), wrapint::get_unsigned_max(width), true);
      if(lbflag) {
        return stnum<Number>(t0, t, true, false);
      } else {
        return stnum<Number>(t, t1, false, true);
      }
    } else {
      tnum_t pos = stnum<Number>::tnum_from_range_s(wrapint::get_unsigned_min(width), ubwrap);
      tnum_t neg = stnum<Number>::tnum_from_range_s(lbwrap, wrapint::get_unsigned_max(width));
      return stnum<Number>(pos, neg, false, false);
    }
    
  }
}

template <typename Number>
tnum<Number> stnum<Number>::tnum_from_range_s(wrapint min, wrapint max){
  wrapint::bitwidth_t w = min.get_bitwidth();
  wrapint chi = min ^ max;
  uint64_t bits = chi.fls();
  

  if(bits > w-1){
    if(min.msb()){
      return tnum<Number>(wrapint::get_signed_min(w),
                          wrapint::get_signed_max(w), false);
    }else{
      return tnum<Number>(wrapint::get_unsigned_min(w), 
                          wrapint::get_signed_max(w), false);
    }
  }
  wrapint delta =(wrapint(1, w) << wrapint(bits, w)) - wrapint(1, w);
  tnum<Number> unsign_tnum(min & (~delta), delta);
  CRAB_LOG("stnum-imply-tnum_from_range_s", crab::outs() << "tnum_from_range_s of "<< min << " and " 
            << max << " is: " << unsign_tnum << "\n";);
  return unsign_tnum;
}

template <typename Number>
stnum<Number> stnum<Number>::normalize(tnum_t a, tnum_t b){
  
  bool flag0 = a.is_bottom();
  bool flag1 = b.is_bottom();
  if (flag1 && flag0){
    return stnum<Number>::bottom();
  }

  bool topa= a.is_top();
  bool topb= b.is_top();
  if (topa || topb) {
    return stnum<Number>::top();
  }else if (flag0 && !flag1) {
    tnum_t t0 = b.getZeroCircle();
    tnum_t t1 = b.getOneCircle();
    return stnum<Number>(t0, t1);
  }else if (!flag0 && flag1) {
    tnum_t t0 = a.getZeroCircle();
    tnum_t t1 = a.getOneCircle();
    return stnum<Number>(t0, t1);
  }else {
    tnum_t t0 = a.getZeroCircle();
    tnum_t t1 = a.getOneCircle();
    t0 = t0 | b.getZeroCircle();
    t1 = t1 | b.getOneCircle();
    return stnum<Number>(t0, t1);
  }
}

template <typename Number>
stnum<Number> stnum<Number>::construct_from_tnum(tnum_t a){
  
  if(a.is_bottom()){
    return stnum<Number>::bottom();
  } 
  
  
  if(a.is_top()){
    return stnum<Number>::top();
  } 

  tnum_t t0 = a.getZeroCircle();
  tnum_t t1 = a.getOneCircle();
  return stnum<Number>(t0, t1);
  
}

template <typename Number>
stnum<Number> stnum<Number>::top() {
  tnum_t t0(wrapint(0, 3), wrapint(3, 3), false);
  tnum_t t1(wrapint(4, 3), wrapint(3, 3), false);
  return stnum<Number>(t0, t1, false, false);
}

template <typename Number>
tnum<Number> stnum<Number>::top_0() {
  tnum_t t0(wrapint(0, 3), wrapint(3, 3), false);
  return t0;
}

template <typename Number>
tnum<Number> stnum<Number>::top_1() {
  tnum_t t1(wrapint(4, 3), wrapint(3, 3), false);
  return t1;
}

template <typename Number>
stnum<Number> stnum<Number>::top(bitwidth_t width) {
  tnum_t t0(wrapint::get_unsigned_min(width), 
      wrapint::get_signed_max(width), false);
  tnum_t t1(wrapint::get_signed_min(width),
      wrapint::get_signed_max(width), false);
  return stnum<Number>(t0, t1, false, false);
}

template <typename Number>
stnum<Number> stnum<Number>::bottom() {
  // the wrapint is irrelevant.
  return stnum<Number>(bottom_0(), bottom_1(), true, true);
}

template <typename Number>
tnum<Number> stnum<Number>::bottom_0() {
  // the wrapint is irrelevant.
  wrapint i(3, 3);
  tnum_t t = tnum<Number>(i, i, true);
  return t;
}

template <typename Number>
tnum<Number> stnum<Number>::bottom_1() {
  // the wrapint is irrelevant.
  wrapint i(7, 3);
  tnum_t t = tnum<Number>(i, i, true);
  return t;
}

template <typename Number>
stnum<Number> stnum<Number>::bottom(bitwidth_t width) {
  // the wrapint is irrelevant.
  tnum_t t0(wrapint::get_signed_max(width), 
      wrapint::get_signed_max(width), true);
  tnum_t t1(wrapint::get_unsigned_max(width),
      wrapint::get_unsigned_max(width), true);
  return stnum<Number>(t0, t1, true, true);
}

/*

// return interval [0111...1, 1000....0]
// In the APLAS'12 paper "signed limit" corresponds to "north pole".
template <typename Number>
stnum<Number>
stnum<Number>::signed_limit(wrapint::bitwidth_t b) {
  return stnum<Number>(wrapint::get_signed_max(b),
                                  wrapint::get_signed_min(b));
}

// return interval [1111...1, 0000....0]
// In the APLAS'12 paper "unsigned limit" corresponds to "south pole".
template <typename Number>
stnum<Number>
stnum<Number>::unsigned_limit(wrapint::bitwidth_t b) {
  return stnum<Number>(wrapint::get_unsigned_max(b),
                                  wrapint::get_unsigned_min(b));
}

template <typename Number>
bool stnum<Number>::cross_signed_limit() const {
  return (signed_limit(get_bitwidth(__LINE__)) <= *this);
}

template <typename Number>
bool stnum<Number>::cross_unsigned_limit() const {
  return (unsigned_limit(get_bitwidth(__LINE__)) <= *this);
}

*/

template <typename Number>
wrapint::bitwidth_t stnum<Number>::get_bitwidth(int line) const {
  bool bottom0 = is_bottom_0();
  bool bottom1 = is_bottom_1();
  if (bottom0 && bottom1) {
    CRAB_ERROR("get_bitwidth() cannot be called from a bottom element at line ",
               line);
  } else if (bottom0 && !bottom1) {
    return tnum_1.get_bitwidth(__LINE__);
  } else if (!bottom0 && bottom1) {
    return tnum_0.get_bitwidth(__LINE__);
  } else {
    if (tnum_0.get_bitwidth(__LINE__) != tnum_1.get_bitwidth(__LINE__)) {
      CRAB_ERROR("inconsistent bitwidths in stnum when get_bitwidth");
    }
    else {
      return tnum_0.get_bitwidth(__LINE__);
    }
  }
}

template <typename Number> tnum<Number> stnum<Number>::get_tnum_0() const {
  return tnum_0;
}

template <typename Number> tnum<Number> stnum<Number>::get_tnum_1() const {
  return tnum_1;
}

/*template <typename Number> void stnum<Number>::set_tnum_0(const tnum_t a )  {
  tnum_0 = a;
}


template <typename Number> void stnum<Number>::set_tnum_1(const tnum_t a )  {
  tnum_1 = a;
}*/

template <typename Number> wrapint stnum<Number>::getSignedMaxValue() const{
  if (!is_bottom_0()) {
    CRAB_LOG("stnum-imply", crab::outs() << "getSignedMaxValue of 1: "
              << tnum_0.value() + tnum_0.mask() <<"\n";);
    return tnum_0.value() + tnum_0.mask();
  } else if (!is_bottom_1()){
    CRAB_LOG("stnum-imply", crab::outs() << "getSignedMaxValue of 2: "
              << tnum_1.value() + tnum_1.mask() <<"\n";);
    return tnum_1.value() + tnum_1.mask();
  } else {
    CRAB_ERROR("method getSignedMaxValue() cannot be called if bottom");
  }
}

template <typename Number> wrapint stnum<Number>::getSignedMinValue() const{
  if (!is_bottom_1()){
    return tnum_1.value();
  } else if (!is_bottom_0()) {
    return tnum_0.value();
  } else {
    CRAB_ERROR("method getSignedMinValue() cannot be called if bottom");
  }
}

template <typename Number> wrapint stnum<Number>::getUnsignedMaxValue() const{
  if (!is_bottom_1()) {
    CRAB_LOG("stnum-imply", crab::outs() << "getUnsignedMaxValue of 1: "
              << tnum_1.value() + tnum_1.mask() <<"\n";);
    return tnum_1.value() + tnum_1.mask();
  } else if (!is_bottom_0()){
    CRAB_LOG("stnum-imply", crab::outs() << "getUnsignedMaxValue of 2: "
              << tnum_0.value() + tnum_0.mask() <<"\n";);
    return tnum_0.value() + tnum_0.mask();
  } else {
    CRAB_ERROR("method getUnsignedMaxValue() cannot be called if bottom");
  }
}

template <typename Number> wrapint stnum<Number>::getUnsignedMinValue() const{
  if (!is_bottom_0()){
    return tnum_0.value();
  } else if (!is_bottom_1()) {
    return tnum_1.value();
  } else {
    CRAB_ERROR("method getUnsignedMinValue() cannot be called if bottom");
  }
}

template <typename Number> bool stnum<Number>::is_bottom() const {
  return (is_bottom_0() && is_bottom_1());
}

template <typename Number> bool stnum<Number>::is_bottom_0() const{
  return m_is_bottom_0 || tnum_0.is_bottom();
}

template <typename Number> bool stnum<Number>::is_bottom_1() const{
  return m_is_bottom_1 || tnum_1.is_bottom();
}

template <typename Number> bool stnum<Number>::is_top() const {
  return is_top_0() && is_top_1(); 
}

template <typename Number> bool stnum<Number>::is_top_0() const {
  wrapint::bitwidth_t w = tnum_0.value().get_bitwidth();
  //wrapint sign_max = wrapint::get_signed_max(w);
  return (tnum_0.value().is_zero() && tnum_0.mask().is_signed_max());
}

template <typename Number> bool stnum<Number>::is_top_1() const {
  wrapint::bitwidth_t w = tnum_1.value().get_bitwidth();
  wrapint sign_min = wrapint::get_signed_min(w);
  wrapint sign_max = wrapint::get_signed_max(w);
  return ((tnum_1.value().is_signed_min()) && (tnum_1.mask().is_signed_max()));
}

template <typename Number> bool stnum<Number>::is_negative() const{
  return is_bottom_0() && !is_bottom_1();
}

template <typename Number> bool stnum<Number>::is_nonnegative() const{
  return !is_bottom_0() && is_bottom_1();
}

template <typename Number> bool stnum<Number>::is_zero() const{
  return is_bottom_1() && tnum_0.is_zero();
}

template <typename Number> bool stnum<Number>::is_positive() const{
  return is_bottom_1() && !tnum_0.value().is_zero();
}

template <typename Number>
ikos::interval<Number> stnum<Number>::to_interval() const {
  using interval_t = ikos::interval<Number>;
  if (is_bottom()) {
    return interval_t::bottom();
  } 
  if (is_top()) {
    return interval_t::top();
  } 
  interval_t i0 = tnum_0.to_interval();
  interval_t i1 = tnum_1.to_interval();
  interval_t res = i0 | i1;
  return res;
}



// need to fix, as from x <= y and y, the result of x may >= y  
// very imprecise, we donot use this in the solver
template <typename Number>
stnum<Number>
stnum<Number>::lower_half_line(bool is_signed) const {
  if (is_top() || is_bottom())
    return *this;

  return stnum<Number>::top();
  
}

// we use this version in the solver to propagate
// make use of x's lb
template <typename Number>
stnum<Number>
stnum<Number>::lower_half_line(const stnum<Number> &x, bool is_signed) const {
  CRAB_LOG("stnum-imply-lower", 
    crab::outs() << "lower_half_line of " << *this << " and " << x << " : \n";);
  if (is_top() || is_bottom())
    return *this;
  wrapint::bitwidth_t w = stnum<Number>::get_bitwidth(__LINE__);

  wrapint xmin_sign = wrapint(0, w) ;
  wrapint xmin_unsign =  wrapint(0, w);

  if(x.is_top()){
    if(is_signed){
      xmin_sign = wrapint::get_signed_min(w);
    }else{
      xmin_unsign = wrapint::get_unsigned_min(w);
    }
  } else if (x.is_bottom()){
    return stnum<Number>::bottom();
  }else{
    xmin_sign = x.getSignedMinValue();
    xmin_unsign = x.getUnsignedMinValue();
  }
  
  CRAB_LOG("stnum-imply-lower", crab::outs() << "lower_half_line 1, xmin_s = " << xmin_sign <<  " : \n";);
  wrapint max_sign = stnum<Number>::getSignedMaxValue();
  wrapint max_unsign = stnum<Number>::getUnsignedMaxValue();
  stnum<Number> res = stnum<Number>::bottom();
  if(is_signed) {
    if((max_sign - wrapint::get_signed_min(w)) < (xmin_sign - wrapint::get_signed_min(w))){
      CRAB_LOG("stnum-imply-lower", crab::outs() << "lower_half_line, res = bottom" << "\n";);
      return stnum<Number>::bottom();
    }
    wrapint lbwrap = xmin_sign;
    wrapint ubwrap = max_sign;
    bool lbflag = lbwrap.msb();
    bool ubflag = ubwrap.msb();
    stnum_t bottom = stnum<Number>::bottom(w);
    CRAB_LOG("stnum-imply-lower", crab::outs() << "lower_half_line 2, lbwrap = " << 
      lbwrap << ", ubwrap = " << ubwrap << " : \n";);
    //need to fix
    if (!(lbflag ^ ubflag)) {
      tnum_t t = stnum<Number>::tnum_from_range_s(lbwrap, ubwrap);
      CRAB_LOG("stnum-imply-lower", crab::outs() << "lower_half_line 3 = "<<  " : \n";);
      if(lbflag){
        res = stnum<Number>(bottom.tnum_0, t, true, false);
      }else{
        res = stnum<Number>(t, bottom.tnum_1, false, true);
      }
    }else{
      CRAB_LOG("stnum-imply-lower", crab::outs() << "lower_half_line 4 = "<< "umin = "<< wrapint::get_unsigned_min(w)<< " : \n";);
      tnum<Number> pos = stnum<Number>::tnum_from_range_s(wrapint::get_unsigned_min(w), ubwrap);
      tnum<Number> neg = stnum<Number>::tnum_from_range_s(lbwrap, wrapint::get_unsigned_max(w));
      CRAB_LOG("stnum-imply-lower", crab::outs() << "lower_half_line 4, pos = "<< pos 
        << ", neg = " << neg <<" : \n";);
      res =  stnum<Number>(pos, neg, false, false);
    }
  }else{
    if (max_unsign < xmin_unsign) return stnum<Number>::bottom();
    tnum_t t = tnum<Number>::tnum_from_range(xmin_unsign, max_unsign);
    res = construct_from_tnum(t);
  }
  CRAB_LOG("stnum-imply-lower", crab::outs() << "lower_half_line, res = " << res << "\n";);
  return res;
}



// need to fix, as from x <= y and y, the result of x may >= y  
// very imprecise, we donot use this in the solver
template <typename Number>
stnum<Number>
stnum<Number>::upper_half_line(bool is_signed) const {
  if (is_top() || is_bottom())
    return *this;

  return stnum<Number>::top();
  
}


// we use this version in the solver to propagate
// make use of x's ub
template <typename Number>
stnum<Number>
stnum<Number>::upper_half_line(const stnum<Number> &x, bool is_signed) const {
  CRAB_LOG("stnum-imply", 
    crab::outs() << "upper_half_line of " << *this << " and " << x << " : \n";);
  if (is_top() || is_bottom())
    return *this;
  wrapint::bitwidth_t w = stnum<Number>::get_bitwidth(__LINE__);

  wrapint xmax_sign = x.tnum_0.value() + x.tnum_0.mask();
  wrapint xmax_unsign = x.tnum_1.value() + x.tnum_1.mask();
  if(x.is_top()){
    if(is_signed){
      xmax_sign = wrapint::get_signed_max(w);
    }else{
      xmax_unsign = wrapint::get_unsigned_max(w);
    }
  }else if(x.is_bottom()){
    return stnum<Number>::bottom();
  }else{
    CRAB_LOG("stnum-imply", crab::outs() << "upper_half_line of else" <<" : \n";);
    xmax_sign = x.getSignedMaxValue();
    xmax_unsign = x.getUnsignedMaxValue();
  }
  CRAB_LOG("stnum-imply", crab::outs() << "upper_half_line of xmax_s = " <<
              xmax_sign << ", xmax_u = " << xmax_unsign <<" : \n";);
  wrapint min_sign = stnum<Number>::getSignedMinValue();
  wrapint min_unsign = stnum<Number>::getUnsignedMinValue();
  if(is_signed) {
    if((min_sign - wrapint::get_signed_min(w)) > (xmax_sign - wrapint::get_signed_min(w))){
      return stnum<Number>::bottom();
    }
    wrapint lbwrap = min_sign;
    wrapint ubwrap = xmax_sign;
    bool lbflag = lbwrap.msb();
    bool ubflag = ubwrap.msb();
    stnum_t bottom = stnum<Number>::bottom(w);
    CRAB_LOG("stnum-imply", crab::outs() << "upper_half_line of 1" <<" : \n";);
    if (!(lbflag ^ ubflag)) {
      tnum_t t = stnum<Number>::tnum_from_range_s(lbwrap, ubwrap);
      if(lbflag){
        return stnum<Number>(bottom.tnum_0, t, true, false);
      }else{
        return stnum<Number>(t, bottom.tnum_1, false, true);
      }
    }else{
      CRAB_LOG("stnum-imply", crab::outs() << "upper_half_line of 2, ubwrap = "
                  << ubwrap << ", lbwrap = " << lbwrap <<" : \n";);
      tnum<Number> pos = stnum<Number>::tnum_from_range_s(wrapint::get_unsigned_min(w), ubwrap);
      CRAB_LOG("stnum-imply", crab::outs() << "upper_half_line of 2 pos =" << pos <<" : \n";);
      tnum<Number> neg = stnum<Number>::tnum_from_range_s(lbwrap, wrapint::get_unsigned_max(w));
      CRAB_LOG("stnum-imply", crab::outs() << "upper_half_line of 2 neg = " << neg <<" : \n";);
      return stnum<Number>(pos, neg, false, false);
    }
  }else{
    if (min_unsign > xmax_unsign) return stnum<Number>::bottom();
    tnum_t t = tnum<Number>::tnum_from_range(min_unsign, xmax_unsign);
    return construct_from_tnum(t);
  }
  
}





template <typename Number> bool stnum<Number>::is_singleton() const {
  if (tnum_0.is_singleton() && is_bottom_1())
   return true;
  else if (tnum_1.is_singleton() && is_bottom_0())
    return true;
  else 
    return false;
}

// determine if the unmask bits is equal to x
template <typename Number> bool stnum<Number>::at(wrapint x) const {
  CRAB_LOG("stnum-imply", 
    crab::outs() <<  x << " at " << *this << " ?: \n";);
  if (is_bottom()) {
    return false;
  } else if (is_top()) {
    return true;
  } else if (x.msb()) {
    return tnum_1.at(x);
  } else {
    return tnum_0.at(x);
  }
}

template <typename Number>
bool stnum<Number>::operator<=(
    const stnum<Number> &x) const {

  CRAB_LOG("stnum-imply", 
    crab::outs() <<  *this << " <= " << x << " ?: \n";);
  bool inclusion0 = false;
  bool inclusion1 = false;
  
  if(x.is_top_0() || is_bottom_0()){
    inclusion0 = true;
  }else if(x.is_bottom_0() || is_top_0()){

    inclusion0 = false;
  }else if(tnum_0 <= x.tnum_0){
    inclusion0 = true;
  }else{
    inclusion0 = false;
  }

  if(x.is_top_1() || is_bottom_1()){
    inclusion1 = true;
  }else if(x.is_bottom_1() || is_top_1()){
    inclusion1 = false;
  }else if(tnum_1 <= x.tnum_1){
    inclusion1 = true;
  }else{
    inclusion1 = false;
  }
  return inclusion0 && inclusion1;
}

template <typename Number>
bool stnum<Number>::operator==(
    const stnum<Number> &x) const {
  return (*this <= x && x <= *this);
}

template <typename Number>
bool stnum<Number>::operator!=(
    const stnum<Number> &x) const {
  return !(this->operator==(x));
}

template <typename Number>
stnum<Number>
stnum<Number>::operator|(const stnum<Number> &x) const {
  CRAB_LOG("stnum-imply-join", 
    crab::outs() << *this << " U " << x << " : \n";);
  if (is_bottom()) {
    return x;
  }
  if (x.is_bottom()) {
    return *this;
  }
  if (is_top() || x.is_top()) {
    return stnum<Number>::top();
  } 
  //wrapint::bitwidth_t width = get_bitwidth(__LINE__);
  tnum<Number> res0 = stnum<Number>::bottom_0();
  tnum<Number> res1 = stnum<Number>::bottom_1();
  
  if(is_bottom_0()){
    res0 = x.tnum_0;
  }else if(x.is_bottom_0()){
    res0 = tnum_0;
  }else{
    res0 = tnum_0 | x.tnum_0;
  }

  if(is_bottom_1()){
    res1 = x.tnum_1;
  }else if(x.is_bottom_1()){
    res1 = tnum_1;
  }else{
    res1 = tnum_1 | x.tnum_1;
  }
  stnum<Number> res(res0, res1);


  /*if(is_bottom_0()){
    res.tnum_0 = x.tnum_0;
  }else if(x.is_bottom_0()){
    res.tnum_0 = tnum_0;
  }else if(!is_top_0() && !x.is_top_0()){
    res.tnum_0 = tnum_0 | x.tnum_0;
  }

 //CRAB_LOG("stnum-imply", 
 //   crab::outs() << "res.tnum_0 = " << res.tnum_0 << "\n";);

  if(is_bottom_1()){
    //CRAB_LOG("stnum-imply", crab::outs() <<"stnum-imply 1" << "\n";);
    res.tnum_1 = x.tnum_1;
  }else if(x.is_bottom_1()){
    //CRAB_LOG("stnum-imply", crab::outs() <<"stnum-imply 2" << "\n";);
    res.tnum_1 = tnum_1;
  }else if(!is_top_1() && !x.is_top_1()){
    //CRAB_LOG("stnum-imply", crab::outs() <<"stnum-imply 3" << "\n";);
    res.tnum_1 = tnum_1 | x.tnum_1;
  }

  //CRAB_LOG("stnum-imply", 
  //  crab::outs() << "res.tnum_1 = " << res.tnum_1 << "\n";);*/
  CRAB_LOG("stnum-imply-join", 
    crab::outs() << "res = " << res << "\n";);
  return res;
}

template <typename Number>
stnum<Number>
stnum<Number>::operator&(const stnum<Number> &x) const {
  CRAB_LOG("stnum-imply-meet", 
    crab::outs() << *this << " M " << x << " : \n";);
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();;
  }
  if (is_top()) {
    return x;
  } else if(x.is_top()){
    return *this;
  }

  tnum_t res0 = stnum<Number>::bottom_0();
  tnum_t res1 = stnum<Number>::bottom_1();

  if(!is_bottom_0() && !x.is_bottom_0()){
    res0 = tnum_0 & x.tnum_0;
  }

  if(!is_bottom_1() && !x.is_bottom_1()){
    res1 = tnum_1 & x.tnum_1;
  }
  stnum<Number> res(res0, res1);

 /* if(is_top_0()){
    res.set_tnum_0(x.tnum_0);
  }else if(x.is_top_0()){
    res.set_tnum_0(tnum_0);
  }else if(!is_bottom_0() && !x.is_bottom_0()){
    res.set_tnum_0(tnum_0 & x.tnum_0);
  }

  /*if(is_top_1()){
    res.set_tnum_1(x.tnum_1);
  }else if(x.is_top_1()){
    CRAB_LOG("stnum-imply-meet", 
    crab::outs() <<"stnum-imply-meet 2: " << (tnum_1 & x.tnum_1) << " \n";);
    res.set_tnum_1(tnum_1);
  }else if(!is_bottom_1() && !x.is_bottom_1()){
    //res.set_tnum_1(tnum_1 & x.tnum_1);
    res.tnum_1 = tnum_1 & x.tnum_1;
  }*/

  CRAB_LOG("stnum-imply-meet", 
    crab::outs() <<"res = " << res << " \n";);
  return res;
}

/* height of lattice is limited width */

template <typename Number>
stnum<Number>
stnum<Number>::operator||(const stnum<Number> &x) const {
   
   // crab::outs() << "stnum-imply-widen "<< *this << " || " << x << " : \n";
  if(is_top()){
    return *this;
  }else if(x.is_top()){
    return x;
  }
  if (is_bottom()) {
    return x;
  } else if (x.is_bottom()) {
    return *this;
  } else {
    tnum<Number> res0 = stnum<Number>::bottom_0();
    tnum<Number> res1 = stnum<Number>::bottom_1();
    /*crab::outs() << "in stnum widen is_bottom_0 = " << is_bottom_0() << 
      ", is_bottom_1 = " << is_bottom_1() << "\n";
    crab::outs() << "in stnum widen x is_bottom_0 = " << x.is_bottom_0() << 
      ", x is_bottom_1 = " << x.is_bottom_1() << "\n";*/
    if(is_top_0()){
      res0 = tnum_0;
    }else if(x.is_top_0()){
      res0 = x.tnum_0;
    }else if(is_bottom_0()){
      res0 = x.tnum_0;
    }else if(x.is_bottom_0()){
      res0 = tnum_0;
    }else{
      uint64_t tr_zero = tnum_0.mask().countr_zero();
      uint64_t ld_zero = tnum_0.mask().countl_zero();
      uint64_t xtr_zero = x.tnum_0.mask().countr_zero();
      uint64_t xld_zero = x.tnum_0.mask().countl_zero();
      if( tr_zero == xtr_zero && (tr_zero == (xld_zero+1)) && tr_zero != 0){
        wrapint common_value = tnum_0.value() & x.tnum_0.value();
        bitwidth_t w = common_value.get_bitwidth(); 
        common_value.clearHighBits(w-tr_zero);
        wrapint mask = wrapint::get_signed_max(w);
        mask.clearLowBits(tr_zero);
        res0  = tnum<Number>(common_value, mask);
        //crab::outs() << "stnum-imply-widen res0 = "<< res0 << "  \n";
      }else{
        //crab::outs() << "stnum-imply-widen 0"<< *this << " || " << x << " : \n";
        //crab::outs() << "stnum-imply-widen 0, tr_zero > xtr_zero, use join \n";
        res0 = tnum_0 | x.tnum_0;
      }
    }

    if(is_top_1()){
      res1 = tnum_1;
    }else if(x.is_top_1()){
      res1 = x.tnum_1;
    }else if(is_bottom_1()){
      res1 = x.tnum_1;
    }else if(x.is_bottom_1()){
      res1 = tnum_1;
    }else{
      uint64_t tr_zero = tnum_1.mask().countr_zero();
      uint64_t ld_zero = tnum_1.mask().countl_zero();
      uint64_t xtr_zero = x.tnum_1.mask().countr_zero();
      uint64_t xld_zero = x.tnum_1.mask().countl_zero();
      if( tr_zero == xtr_zero && (tr_zero == (xld_zero+1)) && tr_zero != 0){
        wrapint common_value = tnum_1.value() & x.tnum_1.value();
        bitwidth_t w = common_value.get_bitwidth(); 
        common_value.clearHighBits(w-tr_zero);
        common_value.setBit(w-1);
        wrapint mask = wrapint::get_signed_max(w);
        mask.clearLowBits(tr_zero);
        res1  = tnum<Number>(common_value, mask);
        //crab::outs() << "stnum-imply-widen res1 = "<< res1 << "  \n";
      }else{
        //crab::outs() << "stnum-imply-widen 1"<< *this << " || " << x << " : \n";
        //crab::outs() << "stnum-imply-widen 1, tr_zero > xtr_zero, use join \n";
        res1 = tnum_1 | x.tnum_1;
      }

      //res1 = tnum_1 || x.tnum_1;
      /*wrapint common_value = tnum_1.value() & x.tnum_1.value();
      tnum<Number> jr = tnum_1 | x.tnum_1;
      wrapint max = jr.value() + jr.mask();
      wrapint::bitwidth_t w = max.get_bitwidth();
      max = max & wrapint::get_signed_max(w);
      
      uint64_t leadingzero = max.countl_zero();
      
      wrapint unsignMax = wrapint::get_unsigned_max(w);
      wrapint unsignMin = wrapint::get_unsigned_min(w);
      unsignMax.clearHighBits(leadingzero);
      wrapint resValue = wrapint::get_signed_min(w);
      //crab::outs() << "stnum-imply-widen , resValue = "<< resValue << ",unsignMax = " << unsignMax << "  \n";
      tnum<Number> res1  = tnum<Number>(resValue, unsignMax);
      //crab::outs() << "stnum-imply-widen res1 = "<< res1 << "  \n";
      tnum<Number> common = tnum<Number>(common_value, unsignMin);
      if(!common_value.is_signed_min()){
        res1 = res1 & common;
      }

      wrapint common_value = tnum_1.value() & x.tnum_1.value();
      wrapint diff = ~common_value;
      res1 = tnum<Number>(common_value, diff);*/
      //crab::outs() << "stnum-imply-widen res1 = "<< res1 << "  \n";
    }

    /*if(res0.is_top()){
      wrapint::bitwidth_t w =  stnum<Number>::get_bitwidth(__LINE__);
      res0 = stnum<Number>::top_0();
    }
    if(res1.is_top()){
      wrapint::bitwidth_t w =  stnum<Number>::get_bitwidth(__LINE__);
      res1 = stnum<Number>::top_1();
    }*/

    stnum<Number> res(res0, res1);
    //CRAB_LOG("stnum-imply-widen", 
    //crab::outs() << "stnum-imply-widen, res0 = " << res0 << ", res1 = "<< res1 <<", res = " << res << " \n";
    return res;

  }
}


// TODO: factorize code with operator||
template <typename Number>
stnum<Number> stnum<Number>::widening_thresholds(
    const stnum<Number> &x, const thresholds<Number> &ts) const {
  return (*this | (x));
}



template <typename Number>
stnum<Number>
stnum<Number>::operator&&(const stnum<Number> &x) const {
  // TODO: for now we call the meet operator.
  return (this->operator&(x));
}


template <typename Number>
stnum<Number>
stnum<Number>::operator+(const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  } 
  
  if (is_top() || x.is_top()) {
    return stnum<Number>::top();
  } 
  
  tnum_t t00 = tnum_0 + x.tnum_0;
  tnum_t t01 = tnum_0 + x.tnum_1;
  tnum_t t10 = tnum_1 + x.tnum_0;
  tnum_t t11 = tnum_1 + x.tnum_1;


  stnum_t same_circle = normalize(t00, t11);
  stnum_t diff_circle = normalize(t01, t10);
  CRAB_LOG("stnum-imply-add", 
    crab::outs() << *this << " + " << x << " = " << (same_circle | diff_circle) << "\n";);
  return same_circle | diff_circle;
}

template <typename Number>
stnum<Number> &
stnum<Number>::operator+=(const stnum<Number> &x) {
  return this->operator=(this->operator+(x));
}

template <typename Number>
stnum<Number> stnum<Number>::operator-() const {
  //wrapint::bitwidth_t w =  get_bitwidth(__LINE__);
  //wrapint sign_min = wrapint::get_signed_min(w);
  //stnum_t res = stnum<Number>::top(w);
  /*if (tnum_1.at(sign_min)) {
    res = stnum<Number>()
  }*/
  CRAB_LOG("stnum-imply", 
    crab::outs() << "-" <<*this << " : \n";);
  if (is_bottom()) {
    return stnum<Number>::bottom();
  } 
  if (is_top()) {
    return stnum<Number>::top();
  } 

  tnum_t t0 = -tnum_0;
  tnum_t t1 = -tnum_1;

  return normalize(t0, t1);
}

template <typename Number>
stnum<Number>
stnum<Number>::operator-(const stnum<Number> &x) const {
  //wrapint::bitwidth_t w =  get_bitwidth(__LINE__);
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " - " << x << " : \n";);
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  } 
  if (is_top() || x.is_top()) {
    return stnum<Number>::top();
  }

  tnum_t t00 = tnum_0 - x.tnum_0;
  tnum_t t10 = tnum_0 - x.tnum_1;
  tnum_t t01 = tnum_1 - x.tnum_0;
  tnum_t t11 = tnum_1 - x.tnum_1;

  stnum_t same_circle = normalize(t01, t10);
  stnum_t diff_circle = normalize(t00, t11);
  CRAB_LOG("stnum-imply-sub", 
    crab::outs() << *this << " - " << x << " = " << (same_circle | diff_circle) << "\n";);
  return same_circle | diff_circle; 
}

template <typename Number>
stnum<Number>
stnum<Number>::operator~() const {
  //wrapint::bitwidth_t w =  get_bitwidth(__LINE__);
  if (is_bottom()) {
    return stnum<Number>::bottom();
  } 
  if (is_top()) {
    return stnum<Number>::top();
  } 
  return stnum<Number>(~tnum_1, ~tnum_0, m_is_bottom_1, m_is_bottom_0);
}

template <typename Number>
stnum<Number> &
stnum<Number>::operator-=(const stnum<Number> &x) {
  return this->operator=(this->operator-(x));
}

template <typename Number>
stnum<Number>
stnum<Number>::operator*(const stnum<Number> &x) const {
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " mul " << x << " : \n";);
  //wrapint::bitwidth_t w =  get_bitwidth(__LINE__);
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return stnum<Number>::top();
  }
  tnum_t t01 = tnum_0 * x.tnum_1;
  tnum_t t00 = tnum_0 * x.tnum_0;
  tnum_t t10 = tnum_1 * x.tnum_0;
  tnum_t t11 = tnum_1 * x.tnum_1;
  stnum_t res = normalize(t00|t11, t01|t10);
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " mul  " << x << " = " << res <<"\n";);
  return res;
}

template <typename Number>
stnum<Number> &
stnum<Number>::operator*=(const stnum<Number> &x) {
  return this->operator=(this->operator*(x));
}




/** division and remainder operations **/
template <typename Number>
stnum<Number>
stnum<Number>::SDiv(const stnum<Number> &x) const {
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " SDiv " << x << " : \n";);
  //wrapint::bitwidth_t w =  get_bitwidth(__LINE__);
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return stnum<Number>::top();
  } 
  tnum_t t01 = tnum_0.SDiv(x.tnum_1);
  tnum_t t00 = tnum_0.SDiv(x.tnum_0);
  tnum_t t10 = tnum_1.SDiv(x.tnum_0);
  tnum_t t11 = tnum_1.SDiv(x.tnum_1);
  stnum_t res = normalize(t00|t11, t01|t10);
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " SDiv  " << x << " = " << res <<"\n";);
  return res;
}


template <typename Number>
stnum<Number>
stnum<Number>::UDiv(const stnum<Number> &x) const {
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " UDiv " << x << " : \n";);
  //wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }
  if (is_top() || x.is_top()) {
    return stnum<Number>::top();
  } 
  tnum_t t01 = tnum_0.UDiv(x.tnum_1);
  tnum_t t00 = tnum_0.UDiv(x.tnum_0);
  tnum_t t10 = tnum_1.UDiv(x.tnum_0);
  tnum_t t11 = tnum_1.UDiv(x.tnum_1);
  stnum_t res = normalize(t00|t01|t11, t10);
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " UDiv  " << x << " = " << res <<"\n";);
  return res;
}

template <typename Number>
stnum<Number>
stnum<Number>::SRem(const stnum<Number> &x) const {
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " SRem " << x << " : \n";);
  //wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  } 
  wrapint::bitwidth_t w = stnum<Number>::get_bitwidth(__LINE__);
  if(is_top() || x.is_top()) {
    return stnum<Number>::top(w);
  } 
  if(x.get_tnum_0().value().is_zero()) {
    return stnum<Number>::top(w);
  }
  tnum_t t01 = tnum_0.SRem(x.tnum_1);
  tnum_t t00 = tnum_0.SRem(x.tnum_0);
  tnum_t t10 = tnum_1.SRem(x.tnum_0);
  tnum_t t11 = tnum_1.SRem(x.tnum_1);
  stnum_t res = normalize(t00|t01, t10|t11);
  return res;
}

template <typename Number>
stnum<Number>
stnum<Number>::URem(const stnum<Number> &x) const {
  CRAB_LOG("stnum-imply", 
    crab::outs() << *this << " URem " << x << " : \n";);
  //wrapint::bitwidth_t w = get_bitwidth(__LINE__);
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  } else if(is_top() || x.is_top()) {
    return stnum<Number>::top();
  } 
  if(x.get_tnum_0().value().is_zero()) {
    return stnum<Number>::top();
  }
  
  tnum_t t01 = tnum_0.URem(x.tnum_1);
  tnum_t t00 = tnum_0.URem(x.tnum_0);
  tnum_t t10 = tnum_1.URem(x.tnum_0);
  tnum_t t11 = tnum_1.URem(x.tnum_1);
  stnum_t res = normalize(t00|t01|t10, t11);
  return res;
}

template <typename Number>
stnum<Number>
stnum<Number>::ZExt(unsigned bits_to_add) const {
  if(is_bottom()){
    return stnum<Number>::bottom();
  }
  if(is_top() ){
    return stnum<Number>::top();
  }
  wrapint::bitwidth_t w = stnum<Number>::get_bitwidth(__LINE__);
  tnum_t t0 = tnum_0 | tnum_1;
  tnum_t t1(wrapint::get_unsigned_max(w),
    wrapint::get_unsigned_max(w), true);
  return stnum<Number>(t0.ZExt(bits_to_add), t1, m_is_bottom_0, true);
}

template <typename Number>
stnum<Number>
stnum<Number>::SExt(unsigned bits_to_add) const {
  if(is_bottom()){
    return stnum<Number>::bottom();
  }
  if(is_top() ){
    return stnum<Number>::top();
  }
  return stnum<Number>(tnum_0.SExt(bits_to_add), tnum_1.SExt(bits_to_add), 
    m_is_bottom_0, m_is_bottom_1);
}

template <typename Number>
stnum<Number>
stnum<Number>::Trunc(unsigned bits_to_keep) const {
  if(is_bottom()){
    return stnum<Number>::bottom();
  }
  if(is_top() ){
    return stnum<Number>::top();
  }
  return stnum<Number>(tnum_0.Trunc(bits_to_keep), tnum_1.Trunc(bits_to_keep), 
    m_is_bottom_0, m_is_bottom_1);
}

/// Shl, LShr, and AShr shifts are treated as unsigned numbers
template <typename Number>
stnum<Number>
stnum<Number>::Shl(const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }else if(is_top() || x.is_top()){
    return stnum<Number>::top();
  }

  if(!x.is_bottom_1()) {
    return stnum<Number>::top();
  }
  return normalize(tnum_0.Shl(x.tnum_0), tnum_1.Shl(x.tnum_0));
}

template <typename Number>
stnum<Number>
stnum<Number>::LShr(const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }else if(is_top() || x.is_top()){
    return stnum<Number>::top();
  }

  if(!x.is_bottom_1()) {
    return stnum<Number>::top();
  }
  wrapint::bitwidth_t width = stnum<Number>::get_bitwidth(__LINE__);
  tnum_t t1(wrapint::get_unsigned_max(width),
      wrapint::get_unsigned_max(width), true);
  return stnum<Number>((tnum_0.LShr(x.tnum_0) | tnum_1.LShr(x.tnum_0)),
    t1);
}

template <typename Number>
stnum<Number>
stnum<Number>::AShr(const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }else if(is_top() || x.is_top()){
    return stnum<Number>::top();
  }
  if(!x.is_bottom_1()) {
    return stnum<Number>::top();
  }
  return stnum<Number>(tnum_0.AShr(x.tnum_0), tnum_1.AShr(x.tnum_0));
}

template <typename Number>
stnum<Number>
stnum<Number>::And(const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }
  if(is_top() || x.is_top()){
    return stnum<Number>::top();
  }
  return stnum<Number>(tnum_0.And(x.tnum_0) | tnum_0.And(x.tnum_1) | tnum_1.And(x.tnum_0)  , tnum_1.And(x.tnum_1));
}

template <typename Number>
stnum<Number>
stnum<Number>::Or(const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }
  if(is_top() || x.is_top()){
    return stnum<Number>::top();
  }
  return stnum<Number>(tnum_0.Or(x.tnum_0), tnum_1.Or(x.tnum_1) | tnum_1.Or(x.tnum_0) | tnum_0.Or(x.tnum_1) );
}

template <typename Number>
stnum<Number>
stnum<Number>::Xor(const stnum<Number> &x) const {
  if (is_bottom() || x.is_bottom()) {
    return stnum<Number>::bottom();
  }
  if(is_top() || x.is_top()){
    return stnum<Number>::top();
  }
  tnum_t t01 = tnum_0.Xor(x.tnum_1);
  tnum_t t00 = tnum_0.Xor(x.tnum_0);
  tnum_t t10 = tnum_1.Xor(x.tnum_0);
  tnum_t t11 = tnum_1.Xor(x.tnum_1);
  return stnum<Number>(t00|t11, t01|t10);
}

template <typename Number>
void stnum<Number>::write(crab::crab_os &o) const {
  if (is_bottom()) {
    o << "_|_";
  } else if (is_top()) {
    o << "top";
  } else {
    o << "{";
    tnum_0.write(o);
    o<<", ";
    tnum_1.write(o);
    o<<"}";
  }
}

} // namespace domains
} // namespace crab
