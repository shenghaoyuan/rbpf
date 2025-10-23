#!/usr/bin/env python3

import json
import sys
import argparse
from typing import Dict, List, Any

def compare_binary_operations(rust_results: List[Dict], cpp_results: List[Dict]) -> List[Dict]:
    """比较二元操作的结果"""
    differences = []
    
    for i, (rust_case, cpp_case) in enumerate(zip(rust_results, cpp_results)):
        # 确保是同一个测试案例
        if (rust_case["operation"] != cpp_case["operation"] or
            rust_case["input_a"] != cpp_case["input_a"] or 
            rust_case["input_b"] != cpp_case["input_b"]):
            print(f"Warning: Mismatched test case at index {i}")
            continue
            
        # 提取 Rust 和 C++ 的结果
        rust_output = None
        cpp_output = None
        
        for result in rust_case["results"]:
            if result["method"].startswith("Rust_"):
                rust_output = result["output"]
                break
                
        for result in cpp_case["results"]:
            if result["method"].startswith("CPP_"):
                cpp_output = result["output"]
                break
        
        if rust_output is None or cpp_output is None:
            print(f"Warning: Missing output for case {i}")
            continue
            
        # 比较结果
        if rust_output != cpp_output:
            differences.append({
                "case_number": i,
                "operation": rust_case["operation"],
                "input_a": rust_case["input_a"],
                "input_b": rust_case["input_b"],
                "rust_output": rust_output,
                "cpp_output": cpp_output,
                "baseline_output": cpp_output,  # C++ 作为基准
                "method": f"Rust_{rust_case['operation']}"
            })
    
    return differences

def compare_at_operations(rust_results: List[Dict], cpp_results: List[Dict]) -> List[Dict]:
    """比较 at 操作的结果"""
    differences = []
    
    for i, (rust_case, cpp_case) in enumerate(zip(rust_results, cpp_results)):
        # 确保是同一个测试案例
        if (rust_case["operation"] != cpp_case["operation"] or
            rust_case["input_interval"] != cpp_case["input_interval"] or 
            rust_case["input_value"] != cpp_case["input_value"]):
            print(f"Warning: Mismatched at test case at index {i}")
            continue
            
        # 提取 Rust 和 C++ 的结果
        rust_output = None
        cpp_output = None
        
        for result in rust_case["results"]:
            if result["method"].startswith("Rust_"):
                rust_output = result["output"]
                break
                
        for result in cpp_case["results"]:
            if result["method"].startswith("CPP_"):
                cpp_output = result["output"]
                break
        
        if rust_output is None or cpp_output is None:
            print(f"Warning: Missing at output for case {i}")
            continue
            
        # 比较结果
        if rust_output != cpp_output:
            differences.append({
                "case_number": i,
                "operation": rust_case["operation"],
                "input_interval": rust_case["input_interval"],
                "input_value": rust_case["input_value"],
                "rust_output": rust_output,
                "cpp_output": cpp_output,
                "baseline_output": cpp_output,  # C++ 作为基准
                "method": f"Rust_{rust_case['operation']}"
            })
    
    return differences

def main():
    parser = argparse.ArgumentParser(description='Compare Rust and C++ wrapped interval test results')
    parser.add_argument('cpp_results', help='Path to C++ results JSON file')
    parser.add_argument('--output', '-o', default='comparison_results.json', 
                       help='Output file for comparison results')
    parser.add_argument('--verbose', '-v', action='store_true', 
                       help='Print detailed comparison information')
    
    args = parser.parse_args()
    
    # 读取 C++ 结果文件
    try:
        with open(args.cpp_results, 'r') as f:
            cpp_data = json.load(f)
    except FileNotFoundError:
        print(f"Error: Could not find C++ results file: {args.cpp_results}")
        sys.exit(1)
    except json.JSONDecodeError:
        print(f"Error: Invalid JSON in C++ results file: {args.cpp_results}")
        sys.exit(1)
    
    # 检查数据结构
    if "binary_operations" not in cpp_data or "at_operations" not in cpp_data:
        print("Error: C++ results file missing required structure")
        sys.exit(1)
    
    # 由于 C++ 结果文件包含了 Rust 的结果，我们需要从中提取
    rust_binary_ops = []
    rust_at_ops = []
    cpp_binary_ops = []
    cpp_at_ops = []
    
    # 分离 Rust 和 C++ 的结果
    for case in cpp_data["binary_operations"]:
        rust_results = [r for r in case["results"] if r["method"].startswith("Rust_")]
        cpp_results = [r for r in case["results"] if r["method"].startswith("CPP_")]
        
        if rust_results and cpp_results:
            rust_case = case.copy()
            rust_case["results"] = rust_results
            rust_binary_ops.append(rust_case)
            
            cpp_case = case.copy()
            cpp_case["results"] = cpp_results
            cpp_binary_ops.append(cpp_case)
    
    for case in cpp_data["at_operations"]:
        rust_results = [r for r in case["results"] if r["method"].startswith("Rust_")]
        cpp_results = [r for r in case["results"] if r["method"].startswith("CPP_")]
        
        if rust_results and cpp_results:
            rust_case = case.copy()
            rust_case["results"] = rust_results
            rust_at_ops.append(rust_case)
            
            cpp_case = case.copy()
            cpp_case["results"] = cpp_results
            cpp_at_ops.append(cpp_case)
    
    # 比较结果
    binary_differences = compare_binary_operations(rust_binary_ops, cpp_binary_ops)
    at_differences = compare_at_operations(rust_at_ops, cpp_at_ops)
    
    # 统计信息
    total_binary_cases = len(rust_binary_ops)
    total_at_cases = len(rust_at_ops)
    binary_failures = len(binary_differences)
    at_failures = len(at_differences)
    
    print(f"\n=== Comparison Results ===")
    print(f"Binary Operations:")
    print(f"  Total cases: {total_binary_cases}")
    print(f"  Differences: {binary_failures}")
    print(f"  Success rate: {((total_binary_cases - binary_failures) / total_binary_cases * 100):.1f}%" if total_binary_cases > 0 else "N/A")
    
    print(f"\nAt Operations:")
    print(f"  Total cases: {total_at_cases}")
    print(f"  Differences: {at_failures}")
    print(f"  Success rate: {((total_at_cases - at_failures) / total_at_cases * 100):.1f}%" if total_at_cases > 0 else "N/A")
    
    # 详细信息
    if args.verbose:
        if binary_differences:
            print(f"\n=== Binary Operation Differences ===")
            for diff in binary_differences[:10]:  # 只显示前10个
                print(f"Case {diff['case_number']}: {diff['operation']}")
                print(f"  Input A: [{diff['input_a']['start']}, {diff['input_a']['end']}], bottom={diff['input_a']['is_bottom']}")
                print(f"  Input B: [{diff['input_b']['start']}, {diff['input_b']['end']}], bottom={diff['input_b']['is_bottom']}")
                print(f"  Rust:    [{diff['rust_output']['start']}, {diff['rust_output']['end']}], bottom={diff['rust_output']['is_bottom']}")
                print(f"  C++:     [{diff['cpp_output']['start']}, {diff['cpp_output']['end']}], bottom={diff['cpp_output']['is_bottom']}")
                print()
        
        if at_differences:
            print(f"\n=== At Operation Differences ===")
            for diff in at_differences[:10]:  # 只显示前10个
                print(f"Case {diff['case_number']}: {diff['operation']}")
                print(f"  Interval: [{diff['input_interval']['start']}, {diff['input_interval']['end']}], bottom={diff['input_interval']['is_bottom']}")
                print(f"  Value: {diff['input_value']}")
                print(f"  Rust: {diff['rust_output']}")
                print(f"  C++:  {diff['cpp_output']}")
                print()
    
    # 保存比较结果
    comparison_results = {
        "summary": {
            "binary_operations": {
                "total_cases": total_binary_cases,
                "differences": binary_failures,
                "success_rate": ((total_binary_cases - binary_failures) / total_binary_cases * 100) if total_binary_cases > 0 else 0
            },
            "at_operations": {
                "total_cases": total_at_cases,
                "differences": at_failures,
                "success_rate": ((total_at_cases - at_failures) / total_at_cases * 100) if total_at_cases > 0 else 0
            }
        },
        "binary_differences": binary_differences,
        "at_differences": at_differences
    }
    
    with open(args.output, 'w') as f:
        json.dump(comparison_results, f, indent=2)
    
    print(f"\nDetailed comparison results saved to: {args.output}")
    
    # 如果有差异，返回非零退出码
    if binary_failures > 0 or at_failures > 0:
        sys.exit(1)

if __name__ == "__main__":
    main()