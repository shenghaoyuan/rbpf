#include <clam/config.h>
#include <clam/CrabDomain.hh>
#include <clam/RegisterAnalysis.hh>
#include "switv_stnum.hh"

namespace clam {
#ifdef INCLUDE_ALL_DOMAINS
REGISTER_DOMAIN(clam::CrabDomain::SWITV_STNUM, switv_stnum_domain)
#else
UNREGISTER_DOMAIN(wrapped_interval_domain)
#endif
} // end namespace clam

