#ifndef _CRAB_CONFIG_H_
#define _CRAB_CONFIG_H_

/** Define whether lin-ldd is available */
/* #undef HAVE_LDD */

/** Define whether apron library is available */
/* #undef HAVE_APRON */

/** Define whether pplite library is available */
/* #undef HAVE_PPLITE */

/** Define whether elina library is available */
/* #undef HAVE_ELINA */

/** Define whether disable logging for debugging purposes */
#define NCRABLOG TRUE

/** Define whether collecting statistics */
#define CRAB_STATS TRUE

/** Use a generic wrapper for abstract domains in tests **/
/* #undef USE_GENERIC_WRAPPER */

#endif
