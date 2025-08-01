#pragma once

/**
 *  Reduced product of sign and constant domains.
 */

#include <crab/domains/abstract_domain.hpp>
#include <crab/domains/backward_assign_operations.hpp>
#include <crab/domains/combined_domains.hpp>
#include <crab/domains/stnum_domain.hpp>
#include <crab/domains/swrapped_interval_domain.hpp>
#include <crab/support/stats.hpp>
#include <crab/numbers/wrapint.hpp>

namespace crab {
namespace domains {

template <typename Number, typename VariableName>
class switv_stnum_domain final
    : public abstract_domain_api<switv_stnum_domain<Number, VariableName>> {

  using switv_stnum_domain_t = switv_stnum_domain<Number, VariableName>;
  using abstract_domain_t = abstract_domain_api<switv_stnum_domain_t>;

public:
  using typename abstract_domain_t::disjunctive_linear_constraint_system_t;
  using typename abstract_domain_t::interval_t;
  using typename abstract_domain_t::linear_constraint_system_t;
  using typename abstract_domain_t::linear_constraint_t;
  using typename abstract_domain_t::linear_expression_t;
  using typename abstract_domain_t::reference_constraint_t;
  using typename abstract_domain_t::variable_or_constant_t;
  using typename abstract_domain_t::variable_t;
  using typename abstract_domain_t::variable_vector_t;
  using typename abstract_domain_t::variable_or_constant_vector_t;    
  using number_t = Number;
  using varname_t = VariableName;

  using stnum_domain_t = stnum_domain<number_t, varname_t>;
  using swrapped_interval_domain_t = swrapped_interval_domain<number_t, varname_t>;
  using stnum_t = typename stnum_domain_t::stnum_t;
  using tnum_t = typename stnum_domain_t::tnum_t;
  using swrapped_interval_t = typename swrapped_interval_domain_t::swrapped_interval_t;

private:
  using reduced_domain_product2_t =
      reduced_domain_product2<number_t, varname_t, swrapped_interval_domain_t, stnum_domain_t>;

  reduced_domain_product2_t m_product;

  switv_stnum_domain(reduced_domain_product2_t &&product)
      : m_product(std::move(product)) {}


/*
tnum_t tnum_from_range_st(wrapint min, wrapint max){
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
  //CRAB_LOG("stnum-imply-tnum_from_range_s", crab::outs() << "tnum_from_range_s of "<< min << " and " 
  //          << max << " is: " << unsign_tnum << "\n";);
  return unsign_tnum;
}*/

  void reduce_variable(const variable_t &v) {
    crab::CrabStats::count(domain_name() + ".count.reduce");
    crab::ScopedCrabStats __st__(domain_name() + ".reduce");
    
    if (is_bottom()) {
      //CRAB_ERROR("reduce_variable meet bottom");
      return;
    }

    swrapped_interval_domain_t &switv_dom = m_product.first();
    stnum_domain_t &stnum_dom = m_product.second();

    swrapped_interval_t sw = switv_dom.get_swrapped_interval(v);
    stnum_t st = stnum_dom.get_stnum(v);
    CRAB_LOG("switv-stnum", crab::outs() << "reduce "<< v << ", begin with sw = " 
              << sw << ", st = " << st << "\n";);

  
    
    tnum_t st0 = st.get_tnum_0();
    tnum_t st1 = st.get_tnum_1();
    
    wrapint start0 = sw.start_0();
    wrapint start1 = sw.start_1();
    wrapint end0 = sw.end_0();
    wrapint end1 = sw.end_1();

    wrapint::bitwidth_t width = start0.get_bitwidth();

    bool sw_is_top = sw.is_top();
    bool st_is_top = st.is_top();
    if(sw_is_top && st_is_top){
      return;
    }else if(sw_is_top && !st_is_top){
      width = st.get_bitwidth(__LINE__);
      //normalize sw to same width
      start0 = wrapint::get_unsigned_min(width);
      end0 = wrapint::get_signed_max(width);
      start1 = wrapint::get_signed_min(width);
      end1 = wrapint::get_unsigned_max(width);
    }else if(!sw_is_top && st_is_top){
      //normalize st to same width
      width = sw.get_bitwidth(__LINE__);
      st0 = tnum<Number>(wrapint::get_unsigned_min(width),
                            wrapint::get_signed_max(width), false);
      st1 = tnum<Number>(wrapint::get_signed_min(width),
                            wrapint::get_signed_max(width), false);
    }else{
      width = st.get_bitwidth(__LINE__);
    }
    CRAB_LOG("switv-stnum", crab::outs() << 
              "after init, strat0 = " << start0 << 
              ", end0 = " << end0 <<
              ", strat1 = " << start1 <<
              ", end1 = " << end1 << "\n";);
    CRAB_LOG("switv-stnum", crab::outs() << 
              "after init, st0 = " << st0 << 
              ", st1 = " << st1  << "\n";);
    
    bool st_reduce0 = false;
    bool sw_reduce0 = false;
    bool st_reduce1 = false;
    bool sw_reduce1 = false;

    /* for 0-circle */
    if(!sw.is_bottom_0() && !st.is_bottom_0()){
      /* We might have learned new bounds from the bits*/
      wrapint st0_min = st0.value();
      wrapint st0_max = st0.value() | st0.mask();
      if(st0_min > start0){
        start0 = st0_min;
        sw_reduce0 = true;
      }
      if(st0_max < end0){
        end0 = st0_max;
        sw_reduce0 = true;
      }
      if(sw_reduce0){
        CRAB_LOG("switv-stnum", crab::outs() << "for 0-circle, update sw to start0 = "
                  << start0 << ", end0 = "  << end0 << "\n";);
      }

      /* We might have learned some bits from the bounds. */
      
      tnum_t range0 = stnum<Number>::tnum_from_range_s(start0, end0);
      CRAB_LOG("switv-stnum", crab::outs() << "tnum_from_range_s of start0 = "<< start0 << ", end0 = " 
              << end0 << ", range0 = " << range0 << "\n";);
      if(st0 != range0){
        st0 = st0 & range0;
        st_reduce0 = true;
      }
      if(st0.is_bottom()){
        CRAB_ERROR("reduce product meet bottom of tnum and tnum_from_range");
      }
      
      if(st_reduce0){
        CRAB_LOG("switv-stnum", crab::outs() << "for 0-circle, update st to st0 = "
                  << st0 << "\n";);
      }
      
      /* Intersecting with the old var_off might have improved our bounds
       * slightly, e.g. if umax was 0x7f...f and var_off was (0; 0xf...fc),
       * then new var_off is (0; 0x7f...fc) which improves our umax.
       */
      st0_min = st0.value();
      st0_max = st0.value() | st0.mask();
      if(st0_min > start0){
        start0 = st0_min;
        sw_reduce0 = true;
      }
      if(st0_max < end0){
        end0 = st0_max;
        sw_reduce0 = true;
      }    
      CRAB_LOG("switv-stnum", crab::outs() << "for 0-circle, update sw snd to start0 = "
                  << start0 << ", end0 = "<< end0 << "\n";);
       
    }else{
      st0 = tnum<Number>(wrapint::get_signed_max(width), wrapint::get_signed_max(width), true);
      start0 = wrapint::get_signed_max(width);
      end0 =  wrapint::get_unsigned_min(width);
      sw_reduce0 = true;
      st_reduce0 = true;
    }

    if(!sw.is_bottom_1() && !st.is_bottom_1()){
      /* We might have learned new bounds from the bits*/
      wrapint st1_min = st1.value();
      wrapint st1_max = st1.value() | st1.mask();
      if(st1_min > start1){
        start1 = st1_min;
        sw_reduce1 = true;
      }
      if(st1_max < end1){
        end1 = st1_max;
        sw_reduce1 = true;
      }
      if(sw_reduce1){
        CRAB_LOG("switv-stnum", crab::outs() << "for 1-circle, update sw to start1 = "
                  << start1 << ", end1 = "  << end1 << "\n";);
      }

      /* for 1-circle, repeat */

      /* We might have learned some bits from the bounds. */
      {
        tnum_t range1 = stnum<Number>::tnum_from_range_s(start1, end1);
        CRAB_LOG("switv-stnum", crab::outs() << "tnum_from_range_s of start1 = "<< start1 << ", end00 = " 
                << end1 << ", range1 = " << range1 << "\n";);
        if(st1 != range1){
          st1 = st1 & range1;
          st_reduce1 = true;
        }
        if(st1.is_bottom()){
          CRAB_ERROR("reduce product meet bottom of tnum and tnum_from_range");
        }
      }
      if(st_reduce1){
        CRAB_LOG("switv-stnum", crab::outs() << "for 1-circle, update st to st1 = "
                  << st1 <<  "\n";);
      }
      
      /* Intersecting with the old var_off might have improved our bounds
       * slightly, e.g. if umax was 0x7f...f and var_off was (0; 0xf...fc),
       * then new var_off is (0; 0x7f...fc) which improves our umax.
       */
      {
        st1_min = st1.value();
        st1_max = st1.value() | st1.mask();
        if(st1_min > start1){
          start1 = st1_min;
          sw_reduce1 = true;
        }
        if(st1_max < end1){
          end1 = st1_max;
          sw_reduce1 = true;
        }  
      }
      if(sw_reduce1){
        CRAB_LOG("switv-stnum", crab::outs() << "for 1-circle, update sw snd to start1 = "
                  << start1 << ", end1 = "  << end1 << "\n";);
      }
         
    }else{
      st1 = tnum<Number>(wrapint::get_unsigned_max(width), wrapint::get_unsigned_max(width), true);
      start1 =  wrapint::get_unsigned_max(width);
      end1 =  wrapint::get_signed_min(width);
      sw_reduce1 = true;
      st_reduce1 = true;
    }
    
    if (st_reduce0 || st_reduce1) {
      CRAB_LOG("switv-stnum", crab::outs() << "reduce "<< v << ", update st to " 
              <<  stnum_t(st0, st1) << "\n";);
      stnum_dom.set(v, stnum_t(st0, st1));
    }
    
    if (sw_reduce0 || sw_reduce1) {

      CRAB_LOG("switv-stnum", crab::outs() << 
              "strat0 = " << start0 << 
              ", end0 = " << end0 <<
              ", strat1 = " << start1 <<
              ", end1 = " << end1 << "\n";);

      swrapped_interval_t res_switv(start0, end0, start1, end1);

      CRAB_LOG("switv-stnum", crab::outs() << "reduce "<< v << ", update sw to " 
              <<  res_switv<< "\n";);
      switv_dom.set(v, res_switv);
    }

  }

public:
  switv_stnum_domain_t make_top() const override {
    reduced_domain_product2_t dom_prod;
    return switv_stnum_domain_t(dom_prod.make_top());
  }

  switv_stnum_domain_t make_bottom() const override {
    reduced_domain_product2_t dom_prod;
    return switv_stnum_domain_t(dom_prod.make_bottom());
  }

  void set_to_top() override {
    reduced_domain_product2_t dom_prod;
    switv_stnum_domain_t abs(dom_prod.make_top());
    std::swap(*this, abs);
  }

  void set_to_bottom() override {
    reduced_domain_product2_t dom_prod;
    switv_stnum_domain_t abs(dom_prod.make_bottom());
    std::swap(*this, abs);
  }

  switv_stnum_domain() : m_product() {}

  switv_stnum_domain(const switv_stnum_domain_t &other) = default;

  switv_stnum_domain(switv_stnum_domain_t &&other) = default;

  switv_stnum_domain_t &
  operator=(const switv_stnum_domain_t &other) = default;

  switv_stnum_domain_t &
  operator=(switv_stnum_domain_t &&other) = default;

  bool is_bottom() const override { return m_product.is_bottom(); }

  bool is_top() const override { return m_product.is_top(); }

  bool operator<=(const switv_stnum_domain_t &other) const override {
    return m_product <= other.m_product;
  }

  void operator|=(const switv_stnum_domain_t &other) override {
    m_product |= other.m_product;
  }

  switv_stnum_domain_t
  operator|(const switv_stnum_domain_t &other) const override {
    return switv_stnum_domain_t(m_product | other.m_product);
  }

  void operator&=(const switv_stnum_domain_t &other) override {
    m_product &= other.m_product;
  }

  switv_stnum_domain_t
  operator&(const switv_stnum_domain_t &other) const override {
    return switv_stnum_domain_t(m_product & other.m_product);
  }

  switv_stnum_domain_t
  operator||(const switv_stnum_domain_t &other) const override {
    //crab::outs() <<"start switv-stnum widen" << " \n";

    crab::outs() << "WIDENING " << *this << " and " << other << " = ";//);
    switv_stnum_domain_t res = switv_stnum_domain_t(m_product || other.m_product);
    crab::outs() << res << " \n";
    return res;
  }

  switv_stnum_domain_t widening_thresholds(
      const switv_stnum_domain_t &other,
      const thresholds<number_t> &ts) const override {
    return switv_stnum_domain_t(
        m_product.widening_thresholds(other.m_product, ts));
  }

  switv_stnum_domain_t
  operator&&(const switv_stnum_domain_t &other) const override {
    return switv_stnum_domain_t(m_product && other.m_product);
  }

  interval_t operator[](const variable_t &v) override {
    return m_product.first()[v] & m_product.second()[v];
  }

  interval_t at(const variable_t &v) const override {
    return m_product.first().at(v) & m_product.second().at(v);
  }
  
  void operator+=(const linear_constraint_system_t &csts) override {
    if (is_bottom())
      return;

    m_product += csts;
    if (!is_bottom()) {
      for (auto const &cst : csts) {
        for (auto const &v : cst.variables()) {
          reduce_variable(v);
          if (is_bottom()) {
            return;
          }
        }
      }
    }
  }

  DEFAULT_ENTAILS(switv_stnum_domain_t)
  
  void operator-=(const variable_t &v) override { m_product -= v; }

  void assign(const variable_t &x, const linear_expression_t &e) override {
    m_product.assign(x, e);
    reduce_variable(x);
  }

  void weak_assign(const variable_t &x, const linear_expression_t &e) override {
    m_product.weak_assign(x, e);
    reduce_variable(x);
  }
  
  void apply(arith_operation_t op, const variable_t &x, const variable_t &y,
             const variable_t &z) override {
    m_product.apply(op, x, y, z);
    reduce_variable(x);
  }

  void apply(arith_operation_t op, const variable_t &x, const variable_t &y,
             number_t k) override {
    m_product.apply(op, x, y, k);
    reduce_variable(x);
  }

  void backward_assign(const variable_t &x, const linear_expression_t &e,
                       const switv_stnum_domain_t &invariant) override {
    m_product.backward_assign(x, e, invariant.m_product);
    // reduce the variables in the right-hand side
    for (auto const &v : e.variables()) {
      reduce_variable(v);
    }
  }

  void backward_apply(arith_operation_t op, const variable_t &x,
                      const variable_t &y, number_t k,
                      const switv_stnum_domain_t &invariant) override {
    m_product.backward_apply(op, x, y, k, invariant.m_product);
    // reduce the variables in the right-hand side
    reduce_variable(y);
  }

  void backward_apply(arith_operation_t op, const variable_t &x,
                      const variable_t &y, const variable_t &z,
                      const switv_stnum_domain_t &invariant) override {
    m_product.backward_apply(op, x, y, z, invariant.m_product);
    // reduce the variables in the right-hand side
    reduce_variable(y);
    reduce_variable(z);
  }

  // cast operators

  void apply(int_conv_operation_t op, const variable_t &dst,
             const variable_t &src) override {
    m_product.apply(op, dst, src);
    reduce_variable(dst);
  }

  // bitwise operators

  void apply(bitwise_operation_t op, const variable_t &x, const variable_t &y,
             const variable_t &z) override {
    m_product.apply(op, x, y, z);
    reduce_variable(x);
  }

  void apply(bitwise_operation_t op, const variable_t &x, const variable_t &y,
             number_t k) override {
    m_product.apply(op, x, y, k);
    reduce_variable(x);
  }

  void select(const variable_t &lhs, const linear_constraint_t &cond,
              const linear_expression_t &e1,
              const linear_expression_t &e2) override {
    m_product.select(lhs, cond, e1, e2);
    reduce_variable(lhs);
  }

  /// switv_stnum_domain implements only standard abstract
  /// operations of a numerical domain so it is intended to be used as
  /// a leaf domain in the hierarchy of domains.
  BOOL_OPERATIONS_NOT_IMPLEMENTED(switv_stnum_domain_t)
  ARRAY_OPERATIONS_NOT_IMPLEMENTED(switv_stnum_domain_t)
  REGION_AND_REFERENCE_OPERATIONS_NOT_IMPLEMENTED(switv_stnum_domain_t)

  void forget(const variable_vector_t &variables) override {
    m_product.forget(variables);
  }

  void project(const variable_vector_t &variables) override {
    m_product.project(variables);
  }

  void expand(const variable_t &var, const variable_t &new_var) override {
    m_product.expand(var, new_var);
  }

  void normalize() override { m_product.normalize(); }

  void minimize() override { m_product.minimize(); }

  void write(crab_os &o) const override { m_product.write(o); }

  linear_constraint_system_t to_linear_constraint_system() const override {
    return m_product.to_linear_constraint_system();
  }

  disjunctive_linear_constraint_system_t
  to_disjunctive_linear_constraint_system() const override {
    return m_product.to_disjunctive_linear_constraint_system();
  }

  std::string domain_name() const override { return m_product.domain_name(); }

  void rename(const variable_vector_t &from,
              const variable_vector_t &to) override {
    m_product.rename(from, to);
  }

  /* begin intrinsics operations */
  void intrinsic(std::string name,
		 const variable_or_constant_vector_t &inputs,
                 const variable_vector_t &outputs) override {
    m_product.intrinsic(name, inputs, outputs);
  }

  void backward_intrinsic(std::string name,
			  const variable_or_constant_vector_t &inputs,
                          const variable_vector_t &outputs,
                          const switv_stnum_domain_t &invariant) override {
    m_product.backward_intrinsic(name, inputs, outputs, invariant.m_product);
  }
  /* end intrinsics operations */

}; // class switv_stnum_domain
} // namespace domains
} // namespace crab

namespace crab {
namespace domains {
template <typename Number, typename VariableName>
struct abstract_domain_traits<switv_stnum_domain<Number, VariableName>> {
  using number_t = Number;
  using varname_t = VariableName;
};
} // namespace domains
} // namespace crab
