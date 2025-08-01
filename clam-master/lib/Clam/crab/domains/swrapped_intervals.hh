#pragma once

#include <crab/domains/swrapped_interval_domain.hpp>
#include "crab_defs.hh"

namespace clam {
using BASE(swrapped_interval_domain_t) =
  crab::domains::swrapped_interval_domain<number_t, region_subdom_varname_t>;
using swrapped_interval_domain_t =
  RGN_FUN(ARRAY_FUN(BOOL_NUM(BASE(swrapped_interval_domain_t))));
} // end namespace clam
