#include <clam/config.h>
#include <clam/CrabDomain.hh>
#include <clam/RegisterAnalysis.hh>
#include "stnum.hh"

namespace clam {
#ifdef INCLUDE_ALL_DOMAINS
REGISTER_DOMAIN(clam::CrabDomain::STNUM, stnum_domain)
#else
UNREGISTER_DOMAIN(stnum_domain)
#endif
} // end namespace clam

