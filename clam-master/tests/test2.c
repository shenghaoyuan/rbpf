extern void abort(void);
extern void __assert_fail(const char *, const char *, unsigned int, const char *) __attribute__ ((__nothrow__ , __leaf__)) __attribute__ ((__noreturn__));
void reach_error() { __assert_fail("0", "bin-suffix-5.c", 3, "reach_error"); }
extern int __VERIFIER_nondet_int();
/*void __VERIFIER_assert(int cond) {
  if (!(cond)) {
    ERROR: {__VERIFIER_error();}
  }
  return;
}*/
int main(void) {
unsigned int y = 0;
  unsigned int x = 5;
  
  while (__VERIFIER_nondet_int()) {
    x += 8;
  }
 //y = (x & 5) - 5;
  __CRAB_intrinsic_print_invariants(x);
  __VERIFIER_assert((x & 5) == 5);
   
  
  return 0;
}
