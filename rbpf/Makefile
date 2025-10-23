CC = gcc
CXX = g++
CFLAGS = -Wall -Wextra -I./include -O2
CXXFLAGS = -Wall -Wextra -std=c++11 -I../clam/crab/include -I../clam/include -O2 -Wno-parentheses
LDFLAGS = -ljson-c
LDFLAGS_CPP = -ljson-c -lrt -lgmp -lgmpxx  

SRC_DIR = tests
BUILD_DIR = ./tests/build
RUST_JSON = ./tests/build/rust_test_cases.json
C_JSON = ./tests/build/c_test_results.json
CPP_JSON = ./tests/build/cpp_test_results.json
WRAPPED_INTERVAL_RUST_JSON = ./tests/build/rust_wrapped_interval_cases.json
WRAPPED_INTERVAL_CPP_JSON = ./tests/build/cpp_wrapped_interval_results.json
CLAM_BUILD_DIR = ../clam/build
CLAM_LIB = $(CLAM_BUILD_DIR)/crab/lib/libCrab.a 

N ?= 100
ITERATIONS ?= 100

# 创建构建目录
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# 编译 clam 库
$(CLAM_LIB):
	mkdir -p $(CLAM_BUILD_DIR)
	cd $(CLAM_BUILD_DIR) && cmake -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DCMAKE_BUILD_TYPE=Release -DLLVM_DIR=/usr/lib/llvm-14/lib/cmake/llvm -DCRAB_BUILD_TESTS=OFF -DCRAB_ROOT=$(realpath ../crab) ..
	cd $(CLAM_BUILD_DIR) && cmake --build . -- -j$(nproc)


# 编译 tnum_test.cpp，并链接 clam 库
$(BUILD_DIR)/tnum_test: $(SRC_DIR)/tnum_test.cpp $(CLAM_LIB) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $< -o $@ $(CLAM_LIB) $(LDFLAGS_CPP)

# 编译 wrapped_interval_test.cpp，并链接 clam 库
$(BUILD_DIR)/wrapped_interval_test: $(SRC_DIR)/wrapped_interval_test.cpp $(CLAM_LIB) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $< -o $@ $(CLAM_LIB) $(LDFLAGS_CPP)

# 生成测试用例（运行test.rs）
$(RUST_JSON): | $(BUILD_DIR)
	cargo run --release --bin tnum_test -- $(ITERATIONS) $(N)

# 生成 wrapped interval 测试用例
$(WRAPPED_INTERVAL_RUST_JSON): | $(BUILD_DIR)
	cargo run --release --bin wrapped_interval_test -- $(N) $(ITERATIONS)

# 执行完整测试流程
test: build rust-test cpp-test compare-results

# 执行 wrapped interval 测试流程
wrapped-interval-test: build wrapped-interval-rust-test wrapped-interval-cpp-test wrapped-interval-compare

build: $(BUILD_DIR)

# 运行Rust测试
rust-test: $(RUST_JSON)
	@echo "Rust test completed, generated test cases: $(RUST_JSON)"

# 运行C++实现测试
cpp-test: $(BUILD_DIR)/tnum_test rust-test
	$(BUILD_DIR)/tnum_test $(RUST_JSON) $(ITERATIONS)
	@echo "C++ tnum test completed, generated results: $(CPP_JSON)"

# 运行 wrapped interval Rust 测试
wrapped-interval-rust-test: $(WRAPPED_INTERVAL_RUST_JSON)
	@echo "Wrapped interval Rust test completed, generated test cases: $(WRAPPED_INTERVAL_RUST_JSON)"

# 运行 wrapped interval C++ 测试
wrapped-interval-cpp-test: $(BUILD_DIR)/wrapped_interval_test wrapped-interval-rust-test
	$(BUILD_DIR)/wrapped_interval_test $(WRAPPED_INTERVAL_RUST_JSON) $(ITERATIONS)
	@echo "C++ wrapped interval test completed, generated results: $(WRAPPED_INTERVAL_CPP_JSON)"

# wrapped interval 比较结果
wrapped-interval-compare: $(WRAPPED_INTERVAL_CPP_JSON) $(WRAPPED_INTERVAL_RUST_JSON)
	@echo "Comparing wrapped interval test results..."
	cargo run --release --bin wrapped_interval_compare -- $(WRAPPED_INTERVAL_CPP_JSON) $(WRAPPED_INTERVAL_RUST_JSON)
	@echo "Wrapped interval test comparison completed"

# 比较结果
compare-results: $(CPP_JSON)
	@echo "Comparing test results..."
	cargo run --release --bin compare -- $<
	@echo "Test comparison completed"

$(CPP_JSON):
	@echo "C++ test results file not found, running cpp-test to generate it..."
	$(MAKE) cpp-test

$(WRAPPED_INTERVAL_CPP_JSON):
	@echo "C++ wrapped interval test results file not found, running wrapped-interval-cpp-test to generate it..."
	$(MAKE) wrapped-interval-cpp-test

# 清理
clean:
	rm -rf $(BUILD_DIR) $(RUST_JSON) $(C_JSON) $(CPP_JSON) $(WRAPPED_INTERVAL_RUST_JSON) $(WRAPPED_INTERVAL_CPP_JSON) $(CLAM_BUILD_DIR)
	cargo clean

# 显示帮助
help:
	@echo "Usage:"
	@echo "  make test [N=100] [ITERATIONS=100]                    - Execute the full tnum test flow"
	@echo "  make wrapped-interval-test [N=100] [ITERATIONS=100]   - Execute the full wrapped interval test flow"
	@echo "  make rust-test [N=100] [ITERATIONS=100]               - Only run Rust tnum tests"
	@echo "  make wrapped-interval-rust-test [N=100] [ITERATIONS=100] - Only run Rust wrapped interval tests"
	@echo "  make cpp-test [ITERATIONS=100]                        - Run C++ tnum baseline tests"
	@echo "  make wrapped-interval-cpp-test [ITERATIONS=100]       - Run C++ wrapped interval baseline tests"
	@echo "  make clean                                             - Clean all generated files"

.PHONY: test rust-test cpp-test compare-results clean help