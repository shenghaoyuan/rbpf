#include <clam/config.h>
#include <clam/CrabDomain.hh>
#include <clam/RegisterAnalysis.hh>
#include "tnum.hh"

namespace clam {
#ifdef INCLUDE_ALL_DOMAINS
REGISTER_DOMAIN(clam::CrabDomain::TNUM, tnum_domain)
#else
UNREGISTER_DOMAIN(tnum_domain)
#endif
} // end namespace clam

