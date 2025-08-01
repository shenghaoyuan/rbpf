#pragma once

#include <crab/domains/switv_stnum_domain.hpp>
#include "crab_defs.hh"

namespace clam {
using BASE(switv_stnum_domain_t) =
  crab::domains::switv_stnum_domain<number_t, region_subdom_varname_t>;
using switv_stnum_domain_t =
  RGN_FUN(ARRAY_FUN(BOOL_NUM(BASE(switv_stnum_domain_t))));
} // end namespace clam
