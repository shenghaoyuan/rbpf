#pragma once

/**
 ** Machine arithmetic interval domain based on the paper
 ** "Signedness-Agnostic Program Analysis: Precise Integer Bounds for
 ** Low-Level Code" by J.A.Navas, P.Schachte, H.Sondergaard, and
 ** P.J.Stuckey published in APLAS'12.
 **/

#include <crab/domains/abstract_domain.hpp>
#include <crab/domains/abstract_domain_specialized_traits.hpp>
#include <crab/domains/combined_domains.hpp>
#include <crab/domains/discrete_domains.hpp>
#include <crab/domains/interval.hpp>
#include <crab/domains/linear_tnum_solver.hpp>
#include <crab/domains/separate_domains.hpp>
#include <crab/domains/tnum.hpp>
#include <crab/numbers/wrapint.hpp>
#include <crab/support/stats.hpp>

#include <boost/optional.hpp>

namespace crab {
namespace domains {

template <typename Number, typename VariableName,
          std::size_t max_reduction_cycles = 10>
class tnum_domain final
    : public abstract_domain_api<
          tnum_domain<Number, VariableName, max_reduction_cycles>> {
  using tnum_domain_t =
      tnum_domain<Number, VariableName, max_reduction_cycles>;
  using abstract_domain_t = abstract_domain_api<tnum_domain_t>;

public:
  using typename abstract_domain_t::disjunctive_linear_constraint_system_t;
  using typename abstract_domain_t::interval_t;
  using typename abstract_domain_t::linear_constraint_system_t;
  using typename abstract_domain_t::linear_constraint_t;
  using typename abstract_domain_t::linear_expression_t;
  using typename abstract_domain_t::reference_constraint_t;
  using typename abstract_domain_t::variable_or_constant_t;
  using typename abstract_domain_t::variable_or_constant_vector_t;
  using typename abstract_domain_t::variable_t;
  using typename abstract_domain_t::variable_vector_t;
  using number_t = Number;
  using varname_t = VariableName;
  using tnum_t = tnum<number_t>;
  using bitwidth_t = typename tnum_t::bitwidth_t;

private:
  using separate_domain_t =
      ikos::separate_domain<variable_t, tnum_t>;
  using solver_t =
      ikos::linear_tnum_solver<number_t, varname_t, separate_domain_t>;

public:
  using iterator = typename separate_domain_t::iterator;

private:
  separate_domain_t _env;

  tnum_domain(separate_domain_t env) : _env(env) {}

  void add(const linear_constraint_system_t &csts,
           std::size_t threshold = max_reduction_cycles) {
    //crab::outs() << "add" << "\n";        
    if (!this->is_bottom()) {
      solver_t solver(csts, threshold);      
      solver.run(this->_env);
    }
  }

  tnum_t eval_expr(const linear_expression_t &expr,
                               bitwidth_t width) {
    if (width == 0) {
      return tnum_t::top();
    }

    tnum_t r =
        tnum_t::mk_tnum(expr.constant(), width);
    for (auto kv : expr) {
      tnum_t c = tnum_t::mk_tnum(kv.first, width);
      // eval_expr should be "const" but operator[] in _env is not marked as
      // "const"
      r += c * this->_env.at(kv.second);
    }
    return r;
  }

public:
  tnum_domain_t make_top() const override {
    return tnum_domain_t(separate_domain_t::top());
  }

  tnum_domain_t make_bottom() const override {
    return tnum_domain_t(separate_domain_t::bottom());
  }

  void set_to_top() override {
    tnum_domain abs(separate_domain_t::top());
    std::swap(*this, abs);
  }

  void set_to_bottom() override {
    tnum_domain abs(separate_domain_t::bottom());
    std::swap(*this, abs);
  }

  tnum_domain() : _env(separate_domain_t::top()) {}

  tnum_domain(const tnum_domain_t &e) : _env(e._env) {
    crab::CrabStats::count(domain_name() + ".count.copy");
    crab::ScopedCrabStats __st__(domain_name() + ".copy");
  }

  tnum_domain_t &operator=(const tnum_domain_t &o) {
    crab::CrabStats::count(domain_name() + ".count.copy");
    crab::ScopedCrabStats __st__(domain_name() + ".copy");
    if (this != &o)
      this->_env = o._env;
    return *this;
  }

  iterator begin() { return this->_env.begin(); }

  iterator end() { return this->_env.end(); }

  bool is_bottom() const override { return this->_env.is_bottom(); }

  bool is_top() const override { return this->_env.is_top(); }

  bool operator<=(const tnum_domain_t &e) const override {
    crab::CrabStats::count(domain_name() + ".count.leq");
    crab::ScopedCrabStats __st__(domain_name() + ".leq");
    // CRAB_LOG("tnum",
    //       crab::outs()<< *this << " <= " << e << "=";);
    bool res = (this->_env <= e._env);
    // CRAB_LOG("tnum",
    //	     crab::outs() << (res ? "yes": "not") << "\n";);
    return res;
  }

  void operator|=(const tnum_domain_t &e) override {
    crab::CrabStats::count(domain_name() + ".count.join");
    crab::ScopedCrabStats __st__(domain_name() + ".join");
    CRAB_LOG("tnum", crab::outs() << *this << " U " << e << " = ");
    this->_env = this->_env | e._env;
    CRAB_LOG("tnum", crab::outs() << *this << "\n";);
  }

  tnum_domain_t
  operator|(const tnum_domain_t &e) const override {
    crab::CrabStats::count(domain_name() + ".count.join");
    crab::ScopedCrabStats __st__(domain_name() + ".join");
    CRAB_LOG("tnum", crab::outs() << *this << " U " << e << " = ");
    tnum_domain_t res(this->_env | e._env);
    CRAB_LOG("tnum", crab::outs() << res << "\n";);
    return res;
  }

  void operator&=(const tnum_domain_t &e) override {
    crab::CrabStats::count(domain_name() + ".count.meet");
    crab::ScopedCrabStats __st__(domain_name() + ".meet");
    CRAB_LOG("tnum", crab::outs() << *this << " M " << e << " = ");
    this->_env = this->_env & e._env;
    CRAB_LOG("tnum", crab::outs() << *this << "\n";);
  }
  
  tnum_domain_t
  operator&(const tnum_domain_t &e) const override {
    crab::CrabStats::count(domain_name() + ".count.meet");
    crab::ScopedCrabStats __st__(domain_name() + ".meet");
    CRAB_LOG("tnum", crab::outs() << *this << " n " << e << " = ");
    tnum_domain_t res(this->_env & e._env);
    CRAB_LOG("tnum", crab::outs() << res << "\n";);
    return res;
  }

  tnum_domain_t
  operator||(const tnum_domain_t &e) const override {
    crab::CrabStats::count(domain_name() + ".count.widening");
    crab::ScopedCrabStats __st__(domain_name() + ".widening");
    CRAB_LOG("tnum",
             crab::outs() << "WIDENING " << *this << " and " << e << " = ");
    tnum_domain_t res(this->_env || e._env);
    CRAB_LOG("tnum", crab::outs() << res << "\n";);
    return res;
  }

  tnum_domain_t
  widening_thresholds(const tnum_domain_t &e,
                      const thresholds<number_t> &ts) const override {
    crab::CrabStats::count(domain_name() + ".count.widening");
    crab::ScopedCrabStats __st__(domain_name() + ".widening");
    CRAB_LOG("tnum",
             crab::outs() << "WIDENING " << *this << " and " << e << " = ");
    tnum_domain_t res(this->_env.widening_thresholds(e._env, ts));
    CRAB_LOG("tnum", crab::outs() << res << "\n";);
    return res;
  }

  tnum_domain_t
  operator&&(const tnum_domain_t &e) const override {
    crab::CrabStats::count(domain_name() + ".count.narrowing");
    crab::ScopedCrabStats __st__(domain_name() + ".narrowing");
    return (this->_env && e._env);
  }

  void operator-=(const variable_t &v) override {
    crab::CrabStats::count(domain_name() + ".count.forget");
    crab::ScopedCrabStats __st__(domain_name() + ".forget");
    this->_env -= v;
  }

  void set(const variable_t &v, tnum_t i) {
    crab::CrabStats::count(domain_name() + ".count.assign");
    crab::ScopedCrabStats __st__(domain_name() + ".assign");
    this->_env.set(v, i);
    CRAB_LOG("tnum", crab::outs()
                                << v << ":=" << i << "=" << _env.at(v) << "\n");
  }

  void set(const variable_t &v, interval_t i) {
    crab::CrabStats::count(domain_name() + ".count.assign");
    crab::ScopedCrabStats __st__(domain_name() + ".assign");
    if (i.lb().is_finite() && i.ub().is_finite()) {
      tnum_t rhs =
	tnum_t::mk_tnum(*(i.lb().number()), *(i.ub().number()),
					 v.get_type().is_integer() ?
					 v.get_type().get_integer_bitwidth() : 0);
      this->_env.set(v, rhs);
      CRAB_LOG("tnum",
               crab::outs() << v << ":=" << i << "=" << _env.at(v) << "\n");
    } else {
      CRAB_WARN(
          "ignored assignment of an open interval in tnum domain");
      *this -= v;
    }
  }

  void set(const variable_t &v, number_t n) {
    crab::CrabStats::count(domain_name() + ".count.assign");
    crab::ScopedCrabStats __st__(domain_name() + ".assign");
    this->_env.set(v, tnum_t::mk_tnum(n,
						       v.get_type().is_integer() ?
						       v.get_type().get_integer_bitwidth() : 0));
    CRAB_LOG("tnum", crab::outs()
                                << v << ":=" << n << "=" << _env.at(v) << "\n");
  }

  // Return unlimited interval
  virtual interval_t operator[](const variable_t &v) override { return at(v); }

  virtual interval_t at(const variable_t &v) const override {
    tnum_t w_i = this->_env.at(v);
    return w_i.to_interval();
  }

  // Return tnum interval
  tnum_t get_tnum(const variable_t &v) const {
    return this->_env.at(v);
  }

  void assign(const variable_t &x, const linear_expression_t &e) override {
    crab::CrabStats::count(domain_name() + ".count.assign");
    crab::ScopedCrabStats __st__(domain_name() + ".assign");
    if (boost::optional<variable_t> v = e.get_variable()) {
      this->_env.set(x, this->_env.at(*v));
    } else {
      tnum_t r = eval_expr(
          e,
          x.get_type().is_integer() ? x.get_type().get_integer_bitwidth() : 0);
      this->_env.set(x, r);
    }
    CRAB_LOG("tnum", crab::outs()
                                << x << ":=" << e << "=" << _env.at(x) << "\n");
  }

  void weak_assign(const variable_t &x, const linear_expression_t &e) override {
    crab::CrabStats::count(domain_name() + ".count.weak_assign");
    crab::ScopedCrabStats __st__(domain_name() + ".weak_assign");
    if (boost::optional<variable_t> v = e.get_variable()) {
      this->_env.join(x, this->_env.at(*v));
    } else {
      tnum_t r = eval_expr(
          e,
          x.get_type().is_integer() ? x.get_type().get_integer_bitwidth() : 0);
      this->_env.join(x, r);
    }
    CRAB_LOG("tnum", crab::outs()
	     << "weak_assign(" << x << "," << e << ")=" << _env.at(x) << "\n");
  }
  
  void apply(arith_operation_t op, const variable_t &x, const variable_t &y,
             const variable_t &z) override {
    crab::CrabStats::count(domain_name() + ".count.apply");
    crab::ScopedCrabStats __st__(domain_name() + ".apply");

    tnum_t yi = _env.at(y);
    tnum_t zi = _env.at(z);
    tnum_t xi = tnum_t::bottom();

    switch (op) {
    case OP_ADDITION:
      xi = yi + zi;
      break;
    case OP_SUBTRACTION:
      xi = yi - zi;
      break;
    case OP_MULTIPLICATION:
      xi = yi * zi;
      break;
    case OP_SDIV:
      xi = yi / zi;
      break;
    case OP_UDIV:
      xi = yi.UDiv(zi);
      break;
    case OP_SREM:
      xi = yi.SRem(zi);
      break;
    case OP_UREM:
      xi = yi.URem(zi);
      break;
    }
    this->_env.set(x, xi);
    CRAB_LOG("tnum", crab::outs() << x << ":=" << y << " " << op << " "
                                         << z << "=" << _env.at(x) << "\n");
  }

  void apply(arith_operation_t op, const variable_t &x, const variable_t &y,
             number_t k) override {
    crab::CrabStats::count(domain_name() + ".count.apply");
    crab::ScopedCrabStats __st__(domain_name() + ".apply");

    tnum_t yi = _env.at(y);
    tnum_t zi = tnum_t::mk_tnum(
        k,
        (x.get_type().is_integer() ? x.get_type().get_integer_bitwidth() : 0));
    tnum_t xi = tnum_t::bottom();

    switch (op) {
    case OP_ADDITION:
      xi = yi + zi;
      break;
    case OP_SUBTRACTION:
      xi = yi - zi;
      break;
    case OP_MULTIPLICATION:
      xi = yi * zi;
      break;
    case OP_SDIV:
      xi = yi / zi;
      break;
    case OP_UDIV:
      xi = yi.UDiv(zi);
      break;
    case OP_SREM:
      xi = yi.SRem(zi);
      break;
    case OP_UREM:
      xi = yi.URem(zi);
      break;
    }
    this->_env.set(x, xi);
    CRAB_LOG("tnum", crab::outs() << x << ":=" << y << " " << op << " "
                                         << k << "=" << _env.at(x) << "\n");
  }

  // cast operations

  void apply(int_conv_operation_t op, const variable_t &dst,
             const variable_t &src) override {
    crab::CrabStats::count(domain_name() + ".count.apply");
    crab::ScopedCrabStats __st__(domain_name() + ".apply");

    tnum_t src_i = this->_env.at(src);
    tnum_t dst_i;

    auto get_bitwidth = [](const variable_t v) {
      auto ty = v.get_type();
      if (!(ty.is_integer() || ty.is_bool())) {
        CRAB_ERROR("unexpected types in cast operation");
      }
      return (ty.is_integer() ? ty.get_integer_bitwidth() : 1);
    };

    if (src_i.is_bottom() || src_i.is_top()) {
      dst_i = src_i;
    } else {
      switch (op) {
      case OP_ZEXT:
      case OP_SEXT: {
        if (get_bitwidth(dst) < get_bitwidth(src)) {
          CRAB_ERROR("destination must be larger than source in sext/zext");
        }
        unsigned bits_to_add = get_bitwidth(dst) - get_bitwidth(src);
        dst_i =
            (op == OP_SEXT ? src_i.SExt(bits_to_add) : src_i.ZExt(bits_to_add));
      } break;
      case OP_TRUNC: {
        if (get_bitwidth(src) < get_bitwidth(dst)) {
          CRAB_ERROR("destination must be smaller than source in truncate");
        }
        unsigned bits_to_keep = get_bitwidth(dst);
        tnum_t dst_i;
        dst_i = src_i.Trunc(bits_to_keep);
      } break;
      }
    }
    set(dst, dst_i);
  }

  // bitwise operations

  void apply(bitwise_operation_t op, const variable_t &x, const variable_t &y,
             const variable_t &z) override {
    crab::CrabStats::count(domain_name() + ".count.apply");
    crab::ScopedCrabStats __st__(domain_name() + ".apply");

    tnum_t yi = _env.at(y);
    tnum_t zi = _env.at(z);
    tnum_t xi = tnum_t::bottom();

    switch (op) {
    case OP_AND: 
      xi = yi.And(zi);
      break;
    case OP_OR: 
      xi = yi.Or(zi);
      break;
    case OP_XOR: 
      xi = yi.Xor(zi);
      break;
    case OP_SHL: 
      xi = yi.Shl(zi);
      break;
    case OP_LSHR: 
      xi = yi.LShr(zi);
      break;
    case OP_ASHR: 
      xi = yi.AShr(zi);
      break;
    }
    this->_env.set(x, xi);
  }

  void apply(bitwise_operation_t op, const variable_t &x, const variable_t &y,
             number_t k) override {
    crab::CrabStats::count(domain_name() + ".count.apply");
    crab::ScopedCrabStats __st__(domain_name() + ".apply");

    tnum_t yi = _env.at(y);
    tnum_t zi = tnum_t::mk_tnum(
        k,
        (x.get_type().is_integer() ? x.get_type().get_integer_bitwidth() : 0));
    tnum_t xi = tnum_t::bottom();
    switch (op) {
    case OP_AND: 
      xi = yi.And(zi);
      break;
    case OP_OR: 
      xi = yi.Or(zi);
      break;
    case OP_XOR: 
      xi = yi.Xor(zi);
      break;
    case OP_SHL: 
      xi = yi.Shl(zi);
      break;
    case OP_LSHR: 
      xi = yi.LShr(zi);
      break;
    case OP_ASHR: 
      xi = yi.AShr(zi);
      break;
    }
    this->_env.set(x, xi);
  }

  void operator+=(const linear_constraint_system_t &csts) override {
    crab::CrabStats::count(domain_name() + ".count.add_constraints");
    crab::ScopedCrabStats __st__(domain_name() + ".add_constraints");
    linear_constraint_system_t wt_csts;
    for (auto const &cst : csts) {
      if (cst.is_well_typed()) {
        wt_csts += cst;
      } else {
        CRAB_WARN(domain_name(), "::add_constraints ignored ", cst,
                  " because it not well typed");
      }
    }

    this->add(wt_csts);
    CRAB_LOG("tnum", crab::outs()
                                << "Added " << csts << " = " << *this << "\n");
  }

  bool entails(const linear_constraint_t &cst) const override {
    if (!cst.is_well_typed()) {
      CRAB_WARN(domain_name(), "::entails ignored ", cst,
		" because it not well typed");
      return false;
    }

    tnum_domain_t copy(*this);
    // REVISIT negation because it might not be sound for machine
    // arithmetic.
    linear_constraint_t neg_cst = cst.negate();
    copy += neg_cst;
    return copy.is_bottom();
  }
  
  // backward arithmetic operations
  void backward_assign(const variable_t &x, const linear_expression_t &e,
                       const tnum_domain_t &inv) override {
    this->operator-=(x);
    CRAB_WARN("Backward assign for tnum not implemented");
  }

  void backward_apply(arith_operation_t op, const variable_t &x,
                      const variable_t &y, number_t z,
                      const tnum_domain_t &inv) override {
    this->operator-=(x);
    CRAB_WARN("Backward apply for tnum not implemented");
  }

  void backward_apply(arith_operation_t op, const variable_t &x,
                      const variable_t &y, const variable_t &z,
                      const tnum_domain_t &inv) override {
    this->operator-=(x);
    CRAB_WARN("Backward apply for tnum not implemented");
  }

  /// tnum_domain implements only standard abstract
  /// operations of a numerical domain so it is intended to be used as
  /// a leaf domain in the hierarchy of domains.
  BOOL_OPERATIONS_NOT_IMPLEMENTED(tnum_domain_t)
  ARRAY_OPERATIONS_NOT_IMPLEMENTED(tnum_domain_t)
  REGION_AND_REFERENCE_OPERATIONS_NOT_IMPLEMENTED(tnum_domain_t)
  DEFAULT_SELECT(tnum_domain_t)
  
  void forget(const variable_vector_t &variables) override {
    if (is_bottom() || is_top()) {
      return;
    }
    for (variable_t var : variables) {
      this->operator-=(var);
    }
  }

  void project(const variable_vector_t &variables) override {
    crab::CrabStats::count(domain_name() + ".count.project");
    crab::ScopedCrabStats __st__(domain_name() + ".project");

    _env.project(variables);
  }

  void expand(const variable_t &x, const variable_t &new_x) override {
    crab::CrabStats::count(domain_name() + ".count.expand");
    crab::ScopedCrabStats __st__(domain_name() + ".expand");

    if (is_bottom() || is_top()) {
      return;
    }

    set(new_x, _env.at(x));
  }

  void normalize() override {}

  void minimize() override {}

  void rename(const variable_vector_t &from,
              const variable_vector_t &to) override {
    crab::CrabStats::count(domain_name() + ".count.rename");
    crab::ScopedCrabStats __st__(domain_name() + ".rename");

    _env.rename(from, to);
  }

  /* begin intrinsics operations */
  void intrinsic(std::string name, const variable_or_constant_vector_t &inputs,
                 const variable_vector_t &outputs) override {
    CRAB_WARN("Intrinsics ", name, " not implemented by ", domain_name());
  }

  void backward_intrinsic(std::string name,
                          const variable_or_constant_vector_t &inputs,
                          const variable_vector_t &outputs,
                          const tnum_domain_t &invariant) override {
    CRAB_WARN("Intrinsics ", name, " not implemented by ", domain_name());
  }
  /* end intrinsics operations */

  void write(crab::crab_os &o) const override {
    crab::CrabStats::count(domain_name() + ".count.write");
    crab::ScopedCrabStats __st__(domain_name() + ".write");

    this->_env.write(o);
  }

  // Important: we make the choice here that we interpret wrapint as
  // signed mathematical integers.
  linear_constraint_system_t to_linear_constraint_system() const override {
    crab::CrabStats::count(domain_name() +
                           ".count.to_linear_constraint_system");
    crab::ScopedCrabStats __st__(domain_name() +
                                 ".to_linear_constraint_system");

    linear_constraint_system_t csts;
    if (this->is_bottom()) {
      csts += linear_constraint_t::get_false();
      return csts;
    }

    for (iterator it = this->_env.begin(); it != this->_env.end(); ++it) {
      variable_t v = it->first;
      tnum_t i = it->second;
      wrapint w_mask = i.mask();
      wrapint w_value = i.value();
      if (w_mask.msb()){
        bitwidth_t w = w_value.get_bitwidth();
        wrapint negmax = wrapint::get_signed_min(w) | w_value;
        wrapint posmax = wrapint::get_signed_max(w) & (w_value + w_mask);
        csts += linear_constraint_t(v >= negmax.get_signed_bignum());
        csts += linear_constraint_t(v <= posmax.get_signed_bignum());
      } else {
        csts += linear_constraint_t(v >= w_value.get_signed_bignum());
        csts += linear_constraint_t(v <= (w_value + w_mask).get_signed_bignum());
      }
    }
    return csts;
  }

  disjunctive_linear_constraint_system_t
  to_disjunctive_linear_constraint_system() const override {
    auto lin_csts = to_linear_constraint_system();
    if (lin_csts.is_false()) {
      return disjunctive_linear_constraint_system_t(true /*is_false*/);
    } else if (lin_csts.is_true()) {
      return disjunctive_linear_constraint_system_t(false /*is_false*/);
    } else {
      return disjunctive_linear_constraint_system_t(lin_csts);
    }
  }

  std::string domain_name() const override { return "Tnums"; }

}; // class tnum_domain

template <typename Number, typename VariableName>
struct abstract_domain_traits<tnum_domain<Number, VariableName>> {
  using number_t = Number;
  using varname_t = VariableName;
};

template <typename Number, typename VariableName>
class constraint_simp_domain_traits<
    tnum_domain<Number, VariableName>> {
public:
  using linear_constraint_t = ikos::linear_constraint<Number, VariableName>;
  using linear_constraint_system_t =
      ikos::linear_constraint_system<Number, VariableName>;

  static void lower_equality(linear_constraint_t cst,
                             linear_constraint_system_t &csts) {
    // We cannot convert an equality into inequalities because we
    // don't know the interpretation (signed/unsigned) for those
    // inequalities.
    csts += cst;
  }
};

} // namespace domains
} // namespace crab

