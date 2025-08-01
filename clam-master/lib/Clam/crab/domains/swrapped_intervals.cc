#include <clam/config.h>
#include <clam/CrabDomain.hh>
#include <clam/RegisterAnalysis.hh>
#include "swrapped_intervals.hh"

namespace clam {
#ifdef INCLUDE_ALL_DOMAINS
REGISTER_DOMAIN(clam::CrabDomain::SWRAPPED_INTERVALS, swrapped_interval_domain)
#else
UNREGISTER_DOMAIN(swrapped_interval_domain)
#endif
} // end namespace clam

