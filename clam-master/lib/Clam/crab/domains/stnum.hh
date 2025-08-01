#pragma once

#include <crab/domains/stnum_domain.hpp>
#include "crab_defs.hh"

namespace clam {
using BASE(stnum_domain_t) =
  crab::domains::stnum_domain<number_t, region_subdom_varname_t>;
using stnum_domain_t =
  RGN_FUN(ARRAY_FUN(BOOL_NUM(BASE(stnum_domain_t))));
} // end namespace clam
