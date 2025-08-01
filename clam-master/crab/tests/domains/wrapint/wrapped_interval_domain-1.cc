#include "../../common.hpp"
#include "../../program_options.hpp"

using namespace std;
using namespace crab;
using namespace crab::cfg;
using namespace crab::cfg_impl;
using namespace crab::domain_impl;

unsigned CrabVerbosity = 0;
z_cfg_t *prog1(variable_factory_t &vfac) {

/* test condition trandformation safety
     x and y are int8
     
     y = -127;
     x = 1;
     if (y >= x) 
        x = 10;
     else
        x = 20;
       
     The expected result at the end is x=[20, 20]
  */
  
  // Defining program variables
  z_var x(vfac["x"], crab::INT_TYPE, 8);
  z_var y(vfac["y"], crab::INT_TYPE, 8);


  // entry and exit block
  auto cfg = new z_cfg_t("entry", "ret");
  // adding blocks
  z_basic_block_t &entry = cfg->insert("entry");
  z_basic_block_t &bb_if = cfg->insert("if");
  z_basic_block_t &bb_then = cfg->insert("then");
  z_basic_block_t &ret = cfg->insert("ret");
  // adding control flow
  entry >> bb_if;
  entry >> bb_then;
  bb_if >> ret;
  bb_then >> ret;
  // adding statements
  entry.assign(y, z_number(-127));
  entry.assign(x, z_number(1));

  bb_if.assume(y >= x);
  bb_if.assign(x, z_number(10));

  bb_then.assume(y <= x-1);
  bb_then.assign(x, z_number(20));
  return cfg;
}

z_cfg_t *prog2(variable_factory_t &vfac) {
  /*
     x= 127;
     x= x+1;
     if (x <= 1) {
       x = 10;
     } else {
       x = -10;
     }

   */
  // Defining program variables
  z_var x(vfac["x"], crab::INT_TYPE, 8);
  // entry and exit block
  auto cfg = new z_cfg_t("entry", "ret");
  // adding blocks
  z_basic_block_t &entry = cfg->insert("entry");
  z_basic_block_t &bb_if = cfg->insert("if");
  z_basic_block_t &bb_then = cfg->insert("then");
  z_basic_block_t &ret = cfg->insert("ret");
  // adding control flow
  entry >> bb_if;
  entry >> bb_then;
  bb_if >> ret;
  bb_then >> ret;
  // adding statements
  entry.assign(x, z_number(127));
  entry.add(x, x, 1);
  z_lin_cst_t c1(x <= z_number(1));
  bb_if.assume(c1);
  bb_if.assign(x, z_number(10));
  z_lin_cst_t c2(x >= z_number(2));
  bb_then.assume(c2);
  bb_then.assign(x, z_number(-10));
  return cfg;
}

int main(int argc, char **argv) {
  bool stats_enabled = false;
  if (!crab_tests::parse_user_options(argc, argv, stats_enabled)) {
    return 0;
  }

  {
    variable_factory_t vfac;
    z_cfg_t *cfg = prog1(vfac);
    crab::outs() << *cfg << "\n";
    {
      // unsound result
      z_interval_domain_t init;
      run(cfg, cfg->entry(), init, false, 1, 2, 20, stats_enabled);
    }
    {
      // sound result
      z_tnum_domain_t init;
      run(cfg, cfg->entry(), init, false, 1, 2, 20, stats_enabled);
    }
    {
      // sound result
      z_wrapped_interval_domain_t init;
      run(cfg, cfg->entry(), init, false, 1, 2, 20, stats_enabled);
    }
    delete cfg;
  }
  {
    variable_factory_t vfac;
    z_cfg_t *cfg = prog2(vfac); 
    crab::outs() << *cfg << "\n";
    z_wrapped_interval_domain_t init;
    run(cfg, cfg->entry(), init, false, 1, 2, 20, stats_enabled);
    delete cfg;
  }
  return 0;
}
