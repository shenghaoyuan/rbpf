# solana-sbpf

SBPF virtual machine + A better verifier

## Our Verifier

- tnum abstract domain (see `src/tnum.rs`)
- `tnum_mul` performance testing (see `tests/tnum.rs`)

To check the performance of mulpti-tnum-mul, just do
```shell
$ make test
...
Total:
function                                        average time(ns)        accuracy
----------------------------------------
tnum_mul                                        150.44                          100.0%
tnum_mul_opt                                    110.68                          100.0%
xtnum_mul_top                                   37848.09                                24.0%
xtnum_mul_high_top                                      1010.27                         94.0%
```
* where `accuracy` represents: if the result of other mul functions is same to `tnum_mul`, then we think it is correct, otherwise incorrect. `accuracy` could be improved using the following four cases:

```rust
// equal, less_than, more_than, not_equal
let ra = tnum_mul a b;
let rb = other_tmum_mul a b;
if ra == rb {
   equal += 1
} else if tnum_in rb ra { // ra in rb
   less_than + = 1
} else if tnum_in ra rb { // rb in ra
   more_than + = 1
} else {
   not_equal + = 1
}
```

## SBPF VM
see [README_OLD](README_OLD.md)
