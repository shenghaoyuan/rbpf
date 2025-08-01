CC = gcc
CXX = g++
CFLAGS = -Wall -Wextra -I./include -O2
CXXFLAGS = -Wall -Wextra -std=c++11 -I../clam-master/crab/include -I../clam-master/include -O2 -Wno-parentheses
LDFLAGS = -ljson-c
LDFLAGS_CPP = -ljson-c -lrt -lgmp -lgmpxx  

SRC_DIR = tests
BUILD_DIR = ./tests/build
RUST_JSON = ./tests/build/rust_test_cases.json
C_JSON = ./tests/build/c_test_results.json
CPP_JSON = ./tests/build/cpp_test_results.json
CLAM_BUILD_DIR = ../clam-master/build
CLAM_LIB = $(CLAM_BUILD_DIR)/crab/lib/libCrab.a 

N ?= 100
ITERATIONS ?= 100

# 创建构建目录
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# 编译 clam-master 库
$(CLAM_LIB):
	mkdir -p $(CLAM_BUILD_DIR)
	cd $(CLAM_BUILD_DIR) && cmake -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DCMAKE_BUILD_TYPE=Release -DLLVM_DIR=/usr/lib/llvm-14/lib/cmake/llvm -DCRAB_BUILD_TESTS=OFF -DCRAB_ROOT=$(realpath ../crab) ..
	cd $(CLAM_BUILD_DIR) && make


# 编译 tnum_test.cpp，并链接 clam-master 库
$(BUILD_DIR)/tnum_test: $(SRC_DIR)/tnum_test.cpp $(CLAM_LIB) | $(BUILD_DIR)
	$(CXX) $(CXXFLAGS) $< -o $@ $(CLAM_LIB) $(LDFLAGS_CPP)

# 生成测试用例（运行test.rs）
$(RUST_JSON): | $(BUILD_DIR)
	cargo run --release --bin tnum_test -- $(ITERATIONS) $(N)

# 执行完整测试流程
test: build rust-test cpp-test compare-results

build: $(BUILD_DIR)

# 运行Rust测试
rust-test: $(RUST_JSON)
	@echo "Rust test completed, generated test cases: $(RUST_JSON)"

# 运行C++实现测试
cpp-test: $(BUILD_DIR)/tnum_test rust-test
	$(BUILD_DIR)/tnum_test $(RUST_JSON) $(ITERATIONS)
	@echo "C++ tnum test completed, generated results: $(CPP_JSON)"

# 比较结果
compare-results: $(CPP_JSON)
	@echo "Comparing test results..."
	cargo run --release --bin compare -- $<
	@echo "Test comparison completed"

$(CPP_JSON):
	@echo "C++ test results file not found, running cpp-test to generate it..."
	$(MAKE) cpp-test

# 清理
clean:
	rm -rf $(BUILD_DIR) $(RUST_JSON) $(C_JSON) $(CPP_JSON) $(CLAM_BUILD_DIR)
	cargo clean

# 显示帮助
help:
	@echo "Usage:"
	@echo "  make test [N=100] [ITERATIONS=100]      - Execute the full test flow (Rust -> C++ -> Compare)"
	@echo "  make rust-test [N=100] [ITERATIONS=100] - Only run Rust tests to generate cases"
	@echo "  make cpp-test [ITERATIONS=100]          - Run C++ baseline tests"
	@echo "  make clean                              - Clean all generated files"

.PHONY: test rust-test cpp-test compare-results clean help