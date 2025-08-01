/*extern void abort(void);
#include <assert.h>
void reach_error() { assert(0); }

extern unsigned int __VERIFIER_nondet_uint(void);
void __VERIFIER_assert(int cond) {
  if (!(cond)) {
    ERROR: {reach_error();abort();}
  }
  return;
}
*/

char main(){
	long long int x, y,z;
	int a = 10;
	char c = 8;
	/*y = -127;
	
	x = 1;
	if (y >= x) 
		x = 10;
	else
		x = 20;*/
	   
	   /*  x= 127;
     x= x+1;
     if (x <= 1) {
       x = 10;
     } else {
       x = -10;
     }*/
     
     y = -1;
     x = 10;
     if(z)	x =y +100;
     else		x = y-100;
     if(x>10) a = a + 50;
     else a = a - 50;
     /*while (x >= y) {
        x = x - y;
     }*/
     x = y >> 1;
     //x = -100 >> z;
	//__CRAB_assert(x==0);
	return c;
	
  
  /*char a = -1;
	char b = a<<10;
	 printf("a = %d\n", a);
	 printf("b = %d\n", b);
	 //printf("c = 1-a = %d\n", c);*
	   y = -10;
     assume(x >= 0 && x <= 100);
     while (x >= y) {
        x = x - y;
     }
	
	 return x;*/
}

