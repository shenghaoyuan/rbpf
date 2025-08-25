### 依赖与环境准备
系统环境:
- Ubuntu-22.04

系统工具:
- make gcc & g++
- cmake
- Rust 工具链 (rustc 和 cargo)
- LLVM 14 (llvm-14-dev, clang-14)

开发库:
- libjson-c-dev
- libgmp-dev

### 安装 clam
```shell
cd clam
sudo apt-get install libboost-all-dev libboost-program-options-dev
sudo apt-get install libgmp-dev
sudo apt-get install libmpfr-dev	
sudo apt-get install libflint-dev

pip3 install lit # update PATH according to the output warnning. vim ~/.bashrc and source ~/.bashrc
pip3 install OutputCheck

mkdir build && cd build
cmake -DCMAKE_INSTALL_PREFIX=$DIR ../                
sudo cmake --build . --target install 
```

### 项目结构
```
.
├── tnum_test/      # 当前仓库
    ├──rbpf
       ├──Makefile
       ├──tests
          ├──tnum_compare.rs
          ├──tnum_test.cpp
          ├──tnum_test.rs
       ├──...
    ├──clam
```

### 工作流
- 生成 Rust 测试用例: 首先会运行 Rust 的测试程序 (tnum_test.rs) 来随机生成一系列测试用例。这些用例以及 Rust 实现的运算结果将被保存到 tests/build/rust_test_cases.json 文件中。

- 编译 CLAM 库: 脚本首先会自动进入 ../clam 目录，使用 cmake 和 make 编译生成 C++ 基准测试所依赖的静态库 libCrab.a。

- 运行 C++ 基准测试: 然后，框架会编译并运行 C++ 的测试程序 (tnum_test.cpp)。该程序会加载上一步生成的 rust_test_cases.json，执行完全相同的运算，并将 C++ 实现的结果输出到 tests/build/cpp_test_results.json。

- 比较和验证: 最后，运行另一个 Rust 程序 (compare)，它会逐一对比两个 JSON 文件中的结果。如果所有结果都一致，则测试通过；否则，它会报告差异，结果保存在`./rbpf/tests/build`下。

### 使用方法

#### 拉取子模块

```
git submodule update --init --recursive
```

#### 构建项目

> 可参考 [./clam/README_NEW.md](./clam/README_NEW.md)

首先到rbpf目录下
```
cd ./rbpf
```
生成随机Tnum对象并用rust版本
```
make rust-test
```
运行clam下的Tnum作为基准测试（这一步编译会很慢）
```
make cpp-test
```
将两种方式得到结果进行对比
```
make compare-result
```
或者直接运行整个工作流程
```
make test [N=100] [ITERATIONS=100]
```
所有测试结果在`./rbpf/tests/build`下
