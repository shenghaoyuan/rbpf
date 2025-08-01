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
extern char nd_char();
int main()
{
    char x;
    x= nd_char();
    __CRAB_assume(x >= 0);
    x =  (x|5) & (-3) ;
    //x=5;
    __CRAB_intrinsic_print_invariants(x);
    char y = -8;
    while (x >= y)
    {
	x = x - y;
    }
    __CRAB_intrinsic_print_invariants(x);
    __VERIFIER_assert(x == -123);

    return 0;
}


