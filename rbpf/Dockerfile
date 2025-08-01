FROM ubuntu:20.04

# 安装基本依赖
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    git \
    libgmp-dev \
    libmpfr-dev \
    libboost-all-dev \
    libjson-c-dev \
    curl \
    clang \
    llvm-14 \
    && rm -rf /var/lib/apt/lists/*

# 安装 Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 编译 clam-master
WORKDIR /opt
COPY ../clam-master /opt/clam-master
WORKDIR /opt/clam-master
RUN mkdir build && cd build \
    && cmake -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DLLVM_DIR=/usr/lib/llvm-14/lib/cmake/llvm \
    -DCRAB_BUILD_TESTS=OFF .. \
    && make -j$(nproc)

# 设置工作目录
WORKDIR /workspace

# 添加说明
RUN echo "Docker environment for rbpf tnum testing" > /README.md
RUN echo "Contains clam-master for tnum implementation comparison" >> /README.md 