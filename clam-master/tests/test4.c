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
int y = nd_int();
  int x = nd_int();
  int z = 0;
  __CRAB_assume(x >=0);
   __CRAB_assume(x <=10);
   __CRAB_assume(y >=5);
   __CRAB_assume(y <=15);
   __CRAB_intrinsic_print_invariants(x);
   __CRAB_intrinsic_print_invariants(y);
   if(y >=x+10 ){
   	   __CRAB_intrinsic_print_invariants(x);
   __CRAB_intrinsic_print_invariants(y);
   	z = 1;
   }
  
  //__VERIFIER_assert((x & 5) == 5);
   
  __CRAB_intrinsic_print_invariants(z);
  return 0;
}
