#pragma once

#include <crab/domains/tnum_domain.hpp>
#include "crab_defs.hh"

namespace clam {
using BASE(tnum_domain_t) =
  crab::domains::tnum_domain<number_t, region_subdom_varname_t>;
using tnum_domain_t =
  RGN_FUN(ARRAY_FUN(BOOL_NUM(BASE(tnum_domain_t))));
} // end namespace clam
