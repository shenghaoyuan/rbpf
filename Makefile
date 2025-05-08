.PHONY: test clean

# default
N ?= 1000
M ?= 1000

test: build
	@echo "Start Testing, generate $(N) Test Cases，each one repeats $(M) times..."
	@./target/release/test_mul $(N) $(M)

build:
	cargo build --release

clean:
	cargo clean
	rm -f mul_test_results.json

help:
	@echo "Usage:"
	@echo "  make test N=1000 ITERATIONS=1000"  
	@echo "  make clean"