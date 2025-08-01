#pragma once

/** Define whether llvm-seahorn is available */
#define HAVE_LLVM_SEAHORN TRUE

/** Whether to use big numbers for representing weights in DBM-based domains **/
/* #undef USE_DBM_BIGNUM */

/** Whether to use safe or unsafe for representing weights
 ** in DBM-based domains. Only if USE_DBM_BIGNUM is disabled.  **/
/* #undef USE_DBM_SAFEINT */

/** Include all default abstract domains.**/
#define INCLUDE_ALL_DOMAINS TRUE

/** whether Clam is compiled as a standalone application **/
#define CLAM_IS_TOPLEVEL TRUE





