#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <stdbool.h>
#include <unistd.h>
#include <stdint.h>
#include <json-c/json.h>
#include <vector>
#include <chrono>

// Crab/CLAM dependencies
#include "crab/numbers/wrapint.hpp"
#include "crab/domains/wrapped_interval.hpp"
#include "crab/domains/wrapped_interval_impl.hpp"

// Dummy implementation for ikos::error
namespace ikos
{
    void error(const char *msg)
    {
        fprintf(stderr, "ikos error: %s\n", msg);
        exit(1);
    }
}

// Dummy implementation for crab::CrabStats
namespace crab
{
    void CrabEnableStats(bool v) {}
    unsigned CrabStats::get(const std::string &n) { return 0; }
    unsigned CrabStats::uset(const std::string &n, unsigned v) { return 0; }
    void CrabStats::count(const std::string &name) {}
    void CrabStats::count_max(const std::string &name, unsigned v) {}
    void CrabStats::start(const std::string &name) {}
    void CrabStats::stop(const std::string &name) {}
    void CrabStats::resume(const std::string &name) {}
    void CrabStats::Print(crab_os &os) {}
    void CrabStats::PrintBrunch(crab_os &os) {}
} // namespace crab

// Dummy class for the Number template parameter
class DummyNumber
{
};

crab::domains::wrapped_interval<DummyNumber> run_cpp_operation(
    const char *operation,
    const crab::domains::wrapped_interval<DummyNumber> &a,
    const crab::domains::wrapped_interval<DummyNumber> &b)
{

    if (strcmp(operation, "add") == 0)
    {
        return a + b;
    }
    else if (strcmp(operation, "sub") == 0)
    {
        return a - b;
    }
    else if (strcmp(operation, "and") == 0)
    {
        return a & b;
    }
    else if (strcmp(operation, "or") == 0)
    {
        return a | b;
    }
    else if (strcmp(operation, "unsigned_mul") == 0)
    {
        // Check for special cases that would cause errors
        if (a.is_bottom() || b.is_bottom()) {
            return crab::domains::wrapped_interval<DummyNumber>::bottom();
        }
        if (a.is_top() || b.is_top()) {
            return crab::domains::wrapped_interval<DummyNumber>::top();
        }
        return a.unsigned_mul(b);
    }
    else if (strcmp(operation, "signed_mul") == 0)
    {
        // Check for special cases that would cause errors
        if (a.is_bottom() || b.is_bottom()) {
            return crab::domains::wrapped_interval<DummyNumber>::bottom();
        }
        if (a.is_top() || b.is_top()) {
            return crab::domains::wrapped_interval<DummyNumber>::top();
        }
        return a.signed_mul(b);
    }
    else if (strcmp(operation, "mul") == 0)
    {
        return a * b;
    }
    else if (strcmp(operation,"signed_div")==0)
    {
        return a.signed_div(b);
    }else if (strcmp(operation, "sdiv") == 0)
    {
        return a.SDiv(b);
    }else if (strcmp(operation, "unsigned_div") == 0)
    {
        return a.unsigned_div(b);
    }    else if (strcmp(operation, "udiv") == 0)
    {
        return a.UDiv(b);
    }
    else if (strcmp(operation, "shl") == 0)
    {
        // Check for special cases
        if (a.is_bottom() || b.is_bottom()) {
            return crab::domains::wrapped_interval<DummyNumber>::bottom();
        }
        if (a.is_top() || b.is_top()) {
            return crab::domains::wrapped_interval<DummyNumber>::top();
        }
        return a.Shl(b);
    }
    

    // Default case: return bottom
    return crab::domains::wrapped_interval<DummyNumber>::bottom();
}

// 处理单参数操作（如trunc）
crab::domains::wrapped_interval<DummyNumber> run_cpp_unary_operation(
    const char *operation,
    const crab::domains::wrapped_interval<DummyNumber> &a,
    uint32_t bits_to_keep)
{
    if (strcmp(operation, "trunc") == 0)
    {
        // 检查是否为top或bottom，避免调用get_bitwidth()错误
        if (a.is_bottom()) {
            return crab::domains::wrapped_interval<DummyNumber>::bottom();
        }
        if (a.is_top()) {
            return crab::domains::wrapped_interval<DummyNumber>::top();
        }
        return a.Trunc(bits_to_keep);
    }
    
    // Default case: return bottom
    return crab::domains::wrapped_interval<DummyNumber>::bottom();
}


// 处理shl_const操作
crab::domains::wrapped_interval<DummyNumber> run_cpp_shl_const_operation(
    const crab::domains::wrapped_interval<DummyNumber> &a,
    uint64_t shift_amount)
{
    if (a.is_bottom()) {
        return crab::domains::wrapped_interval<DummyNumber>::bottom();
    }
    if (a.is_top()) {
        return crab::domains::wrapped_interval<DummyNumber>::top();
    }
    return a.Shl(shift_amount);
}

// 处理lshr_const操作
crab::domains::wrapped_interval<DummyNumber> run_cpp_lshr_const_operation(
    const crab::domains::wrapped_interval<DummyNumber> &a,
    uint64_t shift_amount)
{
    if (a.is_bottom()) {
        return crab::domains::wrapped_interval<DummyNumber>::bottom();
    }
    if (a.is_top()) {
        return crab::domains::wrapped_interval<DummyNumber>::top();
    }
    return a.LShr(shift_amount);
}

bool run_cpp_at_operation(
    const crab::domains::wrapped_interval<DummyNumber> &a,
    uint64_t value)
{
    crab::wrapint wrap_value(value, 64);
    return a.at(wrap_value);
}

bool run_cpp_comparison_operation(
    const char *operation,
    const crab::domains::wrapped_interval<DummyNumber> &a,
    const crab::domains::wrapped_interval<DummyNumber> &b)
{
    if (strcmp(operation, "less_or_equal") == 0)
    {
        return a <= b;
    }
    
    // Default case: return false
    return false;
}

int main(int argc, char *argv[])
{
    if (argc < 2)
    {
        printf("Usage: %s <rust_wrapped_interval_cases.json> [iterations]\n", argv[0]);
        return 1;
    }

    const char *input_file = argv[1];
    const char *output_file = "./tests/build/cpp_wrapped_interval_results.json";
    const int iterations = (argc > 2) ? atoi(argv[2]) : 1000;

    FILE *fp = fopen(input_file, "r");
    if (!fp)
    {
        perror("Failed to open input file");
        return 1;
    }

    fseek(fp, 0, SEEK_END);
    long file_size = ftell(fp);
    fseek(fp, 0, SEEK_SET);

    char *json_str = (char *)malloc(file_size + 1);
    if (!json_str)
    {
        perror("Failed to allocate memory for json file");
        fclose(fp);
        return 1;
    }

    if (fread(json_str, 1, file_size, fp) != (size_t)file_size)
    {
        fprintf(stderr, "Warning: Did not read the full file content.\n");
    }
    json_str[file_size] = '\0';
    fclose(fp);

    struct json_object *root_obj = json_tokener_parse(json_str);
    if (!root_obj)
    {
        fprintf(stderr, "Failed to parse JSON\n");
        free(json_str);
        return 1;
    }

    struct json_object *output_array = json_object_new_object();
    struct json_object *binary_results_array = json_object_new_array();
    struct json_object *at_results_array = json_object_new_array();
    struct json_object *comparison_results_array = json_object_new_array();

    // Process binary operations
    struct json_object *binary_operations;
    if (json_object_object_get_ex(root_obj, "binary_operations", &binary_operations))
    {
        int case_count = json_object_array_length(binary_operations);
        printf("Processing %d binary wrapped interval test cases with C++ implementation...\n", case_count);

        for (int i = 0; i < case_count; i++)
        {
            struct json_object *test_case = json_object_array_get_idx(binary_operations, i);

            // Get operation
            struct json_object *op_obj;
            json_object_object_get_ex(test_case, "operation", &op_obj);
            const char *operation = json_object_get_string(op_obj);

            struct json_object *input_a_obj, *input_b_obj;
            json_object_object_get_ex(test_case, "input_a", &input_a_obj);
            json_object_object_get_ex(test_case, "input_b", &input_b_obj);

            struct json_object *a_start_obj, *a_end_obj, *a_bottom_obj;
            struct json_object *b_start_obj, *b_end_obj, *b_bottom_obj;

            json_object_object_get_ex(input_a_obj, "start", &a_start_obj);
            json_object_object_get_ex(input_a_obj, "end", &a_end_obj);
            json_object_object_get_ex(input_a_obj, "is_bottom", &a_bottom_obj);

            json_object_object_get_ex(input_b_obj, "start", &b_start_obj);
            json_object_object_get_ex(input_b_obj, "end", &b_end_obj);
            json_object_object_get_ex(input_b_obj, "is_bottom", &b_bottom_obj);

            uint64_t a_start = json_object_get_uint64(a_start_obj);
            uint64_t a_end = json_object_get_uint64(a_end_obj);
            bool a_is_bottom = json_object_get_boolean(a_bottom_obj);

            uint64_t b_start = json_object_get_uint64(b_start_obj);
            uint64_t b_end = json_object_get_uint64(b_end_obj);
            bool b_is_bottom = json_object_get_boolean(b_bottom_obj);

            crab::wrapint::bitwidth_t width = 64;

            crab::domains::wrapped_interval<DummyNumber> wint_a, wint_b;
            if (a_is_bottom)
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(a_start, width), crab::wrapint(a_end, width));
            }

            if (b_is_bottom)
            {
                wint_b = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_b = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(b_start, width), crab::wrapint(b_end, width));
            }

            // Perform operation and measure time
            std::vector<double> times;
            crab::domains::wrapped_interval<DummyNumber> result;

            for (int iter = 0; iter < iterations; iter++)
            {
                auto start_time = std::chrono::high_resolution_clock::now();
                result = run_cpp_operation(operation, wint_a, wint_b);
                auto end_time = std::chrono::high_resolution_clock::now();

                auto duration = std::chrono::duration_cast<std::chrono::nanoseconds>(end_time - start_time);
                times.push_back(duration.count());
            }

            double avg_time = 0.0;
            for (double t : times)
            {
                avg_time += t;
            }
            avg_time /= times.size();

            // Create result object
            struct json_object *cpp_result_obj = json_object_new_object();

            std::string method_name = "CPP_" + std::string(operation);
            json_object_object_add(cpp_result_obj, "method",
                                   json_object_new_string(method_name.c_str()));

            struct json_object *output_obj = json_object_new_object();

            if (result.is_bottom())
            {
                json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                json_object_object_add(output_obj, "end", json_object_new_uint64(0));
                json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(true));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(64));
            }
            else if (result.is_top())
            {
                // Handle top case - use maximum range
                json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                json_object_object_add(output_obj, "end", json_object_new_uint64(UINT64_MAX));
                json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(false));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(64));
            }
            else
            {
                // 对于非top/bottom情况，需要检查是否可以安全调用start()和end()
                if (!result.is_top() && !result.is_bottom()) {
                    json_object_object_add(output_obj, "start",
                                           json_object_new_uint64(result.start().get_uint64_t()));
                    json_object_object_add(output_obj, "end",
                                           json_object_new_uint64(result.end().get_uint64_t()));
                } else {
                    // 如果无法安全调用，使用默认值
                    json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                    json_object_object_add(output_obj, "end", json_object_new_uint64(0));
                }
                json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(false));
                json_object_object_add(output_obj, "bitwidth",
                                       json_object_new_int(64)); // 默认64位
            }

            json_object_object_add(cpp_result_obj, "output", output_obj);
            json_object_object_add(cpp_result_obj, "avg_time_ns", json_object_new_double(avg_time));

            // Copy original test case and add cpp result
            struct json_object *new_test_case = json_object_new_object();
            json_object_object_add(new_test_case, "operation", json_object_new_string(operation));
            json_object_object_add(new_test_case, "input_a", json_object_get(input_a_obj));
            json_object_object_add(new_test_case, "input_b", json_object_get(input_b_obj));

            struct json_object *results_array = json_object_new_array();
            struct json_object *original_results;
            json_object_object_get_ex(test_case, "results", &original_results);

            // Copy original results
            int orig_len = json_object_array_length(original_results);
            for (int j = 0; j < orig_len; j++)
            {
                json_object_array_add(results_array,
                                      json_object_get(json_object_array_get_idx(original_results, j)));
            }

            // Add cpp result
            json_object_array_add(results_array, cpp_result_obj);
            json_object_object_add(new_test_case, "results", results_array);

            json_object_array_add(binary_results_array, new_test_case);
        }
    }

    // Process at operations
    struct json_object *at_operations;
    if (json_object_object_get_ex(root_obj, "at_operations", &at_operations))
    {
        int at_case_count = json_object_array_length(at_operations);
        printf("Processing %d at wrapped interval test cases with C++ implementation...\n", at_case_count);

        for (int i = 0; i < at_case_count; i++)
        {
            struct json_object *test_case = json_object_array_get_idx(at_operations, i);

            struct json_object *input_a_obj, *input_value_obj;
            json_object_object_get_ex(test_case, "input_a", &input_a_obj);
            json_object_object_get_ex(test_case, "input_value", &input_value_obj);

            struct json_object *a_start_obj, *a_end_obj, *a_bottom_obj;
            json_object_object_get_ex(input_a_obj, "start", &a_start_obj);
            json_object_object_get_ex(input_a_obj, "end", &a_end_obj);
            json_object_object_get_ex(input_a_obj, "is_bottom", &a_bottom_obj);

            uint64_t a_start = json_object_get_uint64(a_start_obj);
            uint64_t a_end = json_object_get_uint64(a_end_obj);
            bool a_is_bottom = json_object_get_boolean(a_bottom_obj);
            uint64_t test_value = json_object_get_uint64(input_value_obj);

            crab::wrapint::bitwidth_t width = 64;

            crab::domains::wrapped_interval<DummyNumber> wint_a;
            if (a_is_bottom)
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(a_start, width), crab::wrapint(a_end, width));
            }

            // Perform at operation and measure time
            std::vector<double> times;
            bool result = false;

            for (int iter = 0; iter < iterations; iter++)
            {
                auto start_time = std::chrono::high_resolution_clock::now();
                result = run_cpp_at_operation(wint_a, test_value);
                auto end_time = std::chrono::high_resolution_clock::now();

                auto duration = std::chrono::duration_cast<std::chrono::nanoseconds>(end_time - start_time);
                times.push_back(duration.count());
            }

            double avg_time = 0.0;
            for (double t : times)
            {
                avg_time += t;
            }
            avg_time /= times.size();

            // Create result object
            struct json_object *cpp_at_result_obj = json_object_new_object();
            json_object_object_add(cpp_at_result_obj, "method",
                                   json_object_new_string("CPP_at"));
            json_object_object_add(cpp_at_result_obj, "output", json_object_new_boolean(result));
            json_object_object_add(cpp_at_result_obj, "avg_time_ns", json_object_new_double(avg_time));

            // Copy original test case and add cpp result
            struct json_object *new_at_test_case = json_object_new_object();
            json_object_object_add(new_at_test_case, "operation", json_object_new_string("at"));
            json_object_object_add(new_at_test_case, "input_a", json_object_get(input_a_obj));
            json_object_object_add(new_at_test_case, "input_value", json_object_get(input_value_obj));

            struct json_object *test_results_array = json_object_new_array();
            struct json_object *original_at_results;
            json_object_object_get_ex(test_case, "results", &original_at_results);

            // Copy original results
            int orig_at_len = json_object_array_length(original_at_results);
            for (int j = 0; j < orig_at_len; j++)
            {
                json_object_array_add(test_results_array,
                                      json_object_get(json_object_array_get_idx(original_at_results, j)));
            }

            // Add cpp result
            json_object_array_add(test_results_array, cpp_at_result_obj);
            json_object_object_add(new_at_test_case, "results", test_results_array);

            json_object_array_add(at_results_array, new_at_test_case);
        }
    }

    // Process comparison operations
    struct json_object *comparison_operations;
    if (json_object_object_get_ex(root_obj, "comparison_operations", &comparison_operations))
    {
        int comp_case_count = json_object_array_length(comparison_operations);
        printf("Processing %d comparison wrapped interval test cases with C++ implementation...\n", comp_case_count);

        for (int i = 0; i < comp_case_count; i++)
        {
            struct json_object *test_case = json_object_array_get_idx(comparison_operations, i);

            // Get operation
            struct json_object *op_obj;
            json_object_object_get_ex(test_case, "operation", &op_obj);
            const char *operation = json_object_get_string(op_obj);

            struct json_object *input_a_obj, *input_b_obj;
            json_object_object_get_ex(test_case, "input_a", &input_a_obj);
            json_object_object_get_ex(test_case, "input_b", &input_b_obj);

            struct json_object *a_start_obj, *a_end_obj, *a_bottom_obj;
            struct json_object *b_start_obj, *b_end_obj, *b_bottom_obj;

            json_object_object_get_ex(input_a_obj, "start", &a_start_obj);
            json_object_object_get_ex(input_a_obj, "end", &a_end_obj);
            json_object_object_get_ex(input_a_obj, "is_bottom", &a_bottom_obj);

            json_object_object_get_ex(input_b_obj, "start", &b_start_obj);
            json_object_object_get_ex(input_b_obj, "end", &b_end_obj);
            json_object_object_get_ex(input_b_obj, "is_bottom", &b_bottom_obj);

            uint64_t a_start = json_object_get_uint64(a_start_obj);
            uint64_t a_end = json_object_get_uint64(a_end_obj);
            bool a_is_bottom = json_object_get_boolean(a_bottom_obj);

            uint64_t b_start = json_object_get_uint64(b_start_obj);
            uint64_t b_end = json_object_get_uint64(b_end_obj);
            bool b_is_bottom = json_object_get_boolean(b_bottom_obj);

            crab::wrapint::bitwidth_t width = 64;

            crab::domains::wrapped_interval<DummyNumber> wint_a, wint_b;
            if (a_is_bottom)
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(a_start, width), crab::wrapint(a_end, width));
            }

            if (b_is_bottom)
            {
                wint_b = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_b = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(b_start, width), crab::wrapint(b_end, width));
            }

            // Perform comparison operation and measure time
            std::vector<double> times;
            bool result = false;

            for (int iter = 0; iter < iterations; iter++)
            {
                auto start_time = std::chrono::high_resolution_clock::now();
                result = run_cpp_comparison_operation(operation, wint_a, wint_b);
                auto end_time = std::chrono::high_resolution_clock::now();

                auto duration = std::chrono::duration_cast<std::chrono::nanoseconds>(end_time - start_time);
                times.push_back(duration.count());
            }

            double avg_time = 0.0;
            for (double t : times)
            {
                avg_time += t;
            }
            avg_time /= times.size();

            // Create result object
            struct json_object *cpp_comp_result_obj = json_object_new_object();
            std::string method_name = "CPP_" + std::string(operation);
            json_object_object_add(cpp_comp_result_obj, "method",
                                   json_object_new_string(method_name.c_str()));
            json_object_object_add(cpp_comp_result_obj, "output", json_object_new_boolean(result));
            json_object_object_add(cpp_comp_result_obj, "avg_time_ns", json_object_new_double(avg_time));

            // Copy original test case and add cpp result
            struct json_object *new_comp_test_case = json_object_new_object();
            json_object_object_add(new_comp_test_case, "operation", json_object_new_string(operation));
            json_object_object_add(new_comp_test_case, "input_a", json_object_get(input_a_obj));
            json_object_object_add(new_comp_test_case, "input_b", json_object_get(input_b_obj));

            struct json_object *comp_results_array = json_object_new_array();
            struct json_object *original_comp_results;
            json_object_object_get_ex(test_case, "results", &original_comp_results);

            // Copy original results
            int orig_comp_len = json_object_array_length(original_comp_results);
            for (int j = 0; j < orig_comp_len; j++)
            {
                json_object_array_add(comp_results_array,
                                      json_object_get(json_object_array_get_idx(original_comp_results, j)));
            }

            // Add cpp result
            json_object_array_add(comp_results_array, cpp_comp_result_obj);
            json_object_object_add(new_comp_test_case, "results", comp_results_array);

            json_object_array_add(comparison_results_array, new_comp_test_case);
        }
    }

    // Process unary operations (like trunc)
    struct json_object *unary_operations;
    struct json_object *unary_results_array = json_object_new_array();
    if (json_object_object_get_ex(root_obj, "unary_operations", &unary_operations))
    {
        int unary_case_count = json_object_array_length(unary_operations);
        printf("Processing %d unary wrapped interval test cases with C++ implementation...\n", unary_case_count);

        for (int i = 0; i < unary_case_count; i++)
        {
            struct json_object *test_case = json_object_array_get_idx(unary_operations, i);

            // Get operation
            struct json_object *op_obj;
            json_object_object_get_ex(test_case, "operation", &op_obj);
            const char *operation = json_object_get_string(op_obj);

            struct json_object *input_a_obj, *bits_to_keep_obj;
            json_object_object_get_ex(test_case, "input_a", &input_a_obj);
            json_object_object_get_ex(test_case, "bits_to_keep", &bits_to_keep_obj);

            struct json_object *a_start_obj, *a_end_obj, *a_bottom_obj;
            json_object_object_get_ex(input_a_obj, "start", &a_start_obj);
            json_object_object_get_ex(input_a_obj, "end", &a_end_obj);
            json_object_object_get_ex(input_a_obj, "is_bottom", &a_bottom_obj);

            uint64_t a_start = json_object_get_uint64(a_start_obj);
            uint64_t a_end = json_object_get_uint64(a_end_obj);
            bool a_is_bottom = json_object_get_boolean(a_bottom_obj);
            uint32_t bits_to_keep = json_object_get_int(bits_to_keep_obj);

            crab::wrapint::bitwidth_t width = 64;

            crab::domains::wrapped_interval<DummyNumber> wint_a;
            if (a_is_bottom)
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(a_start, width), crab::wrapint(a_end, width));
            }

            // Perform unary operation and measure time
            std::vector<double> times;
            crab::domains::wrapped_interval<DummyNumber> result;

            for (int iter = 0; iter < iterations; iter++)
            {
                auto start_time = std::chrono::high_resolution_clock::now();
                result = run_cpp_unary_operation(operation, wint_a, bits_to_keep);
                auto end_time = std::chrono::high_resolution_clock::now();

                auto duration = std::chrono::duration_cast<std::chrono::nanoseconds>(end_time - start_time);
                times.push_back(duration.count());
            }

            // Calculate average time
            double avg_time = 0.0;
            for (double t : times)
            {
                avg_time += t;
            }
            avg_time /= times.size();

            // Create result object
            struct json_object *cpp_unary_result_obj = json_object_new_object();
            json_object_object_add(cpp_unary_result_obj, "method", json_object_new_string("CPP_trunc"));
            json_object_object_add(cpp_unary_result_obj, "avg_time_ns", json_object_new_double(avg_time));

            struct json_object *output_obj = json_object_new_object();
            
            // 检查是否为top或bottom状态
            if (result.is_bottom()) {
                json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                json_object_object_add(output_obj, "end", json_object_new_uint64(0));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(64)); // 默认64位
            } else if (result.is_top()) {
                // 对于top状态，使用最大值
                uint64_t max_val = UINT64_MAX; // 64位最大值
                json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                json_object_object_add(output_obj, "end", json_object_new_uint64(max_val));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(64)); // 默认64位
            } else {
                // 对于非top/bottom情况，需要检查是否可以安全调用start()和end()
                if (!result.is_top() && !result.is_bottom()) {
                    json_object_object_add(output_obj, "start", json_object_new_uint64(result.start().get_uint64_t()));
                    json_object_object_add(output_obj, "end", json_object_new_uint64(result.end().get_uint64_t()));
                } else {
                    // 如果无法安全调用，使用默认值
                    json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                    json_object_object_add(output_obj, "end", json_object_new_uint64(0));
                }
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(64)); // 默认64位
            }
            json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(result.is_bottom()));
            json_object_object_add(cpp_unary_result_obj, "output", output_obj);

            // Create test case object
            struct json_object *new_unary_test_case = json_object_new_object();
            json_object_object_add(new_unary_test_case, "operation", json_object_new_string(operation));
            json_object_object_add(new_unary_test_case, "input_a", input_a_obj);
            json_object_object_add(new_unary_test_case, "bits_to_keep", bits_to_keep_obj);

            struct json_object *unary_test_results_array = json_object_new_array();
            json_object_array_add(unary_test_results_array, cpp_unary_result_obj);
            json_object_object_add(new_unary_test_case, "results", unary_test_results_array);

            json_object_array_add(unary_results_array, new_unary_test_case);
        }
    }


    // Process shl_const operations
    struct json_object *shl_const_operations;
    struct json_object *shl_const_results_array = json_object_new_array();
    if (json_object_object_get_ex(root_obj, "shl_const_operations", &shl_const_operations))
    {
        int shl_const_case_count = json_object_array_length(shl_const_operations);
        printf("Processing %d shl_const wrapped interval test cases with C++ implementation...\n", shl_const_case_count);

        for (int i = 0; i < shl_const_case_count; i++)
        {
            struct json_object *test_case = json_object_array_get_idx(shl_const_operations, i);

            // Get operation
            struct json_object *op_obj;
            json_object_object_get_ex(test_case, "operation", &op_obj);
            const char *operation = json_object_get_string(op_obj);

            struct json_object *input_a_obj, *shift_amount_obj;
            json_object_object_get_ex(test_case, "input_a", &input_a_obj);
            json_object_object_get_ex(test_case, "shift_amount", &shift_amount_obj);

            struct json_object *a_start_obj, *a_end_obj, *a_bottom_obj;
            json_object_object_get_ex(input_a_obj, "start", &a_start_obj);
            json_object_object_get_ex(input_a_obj, "end", &a_end_obj);
            json_object_object_get_ex(input_a_obj, "is_bottom", &a_bottom_obj);

            uint64_t a_start = json_object_get_uint64(a_start_obj);
            uint64_t a_end = json_object_get_uint64(a_end_obj);
            bool a_is_bottom = json_object_get_boolean(a_bottom_obj);
            uint64_t shift_amount = json_object_get_uint64(shift_amount_obj);

            crab::wrapint::bitwidth_t width = 64;

            crab::domains::wrapped_interval<DummyNumber> wint_a;
            if (a_is_bottom)
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(a_start, width), crab::wrapint(a_end, width));
            }

            // Perform shl_const operation and measure time
            std::vector<double> times;
            crab::domains::wrapped_interval<DummyNumber> result;

            for (int iter = 0; iter < iterations; iter++)
            {
                auto start_time = std::chrono::high_resolution_clock::now();
                result = run_cpp_shl_const_operation(wint_a, shift_amount);
                auto end_time = std::chrono::high_resolution_clock::now();

                auto duration = std::chrono::duration_cast<std::chrono::nanoseconds>(end_time - start_time);
                times.push_back(duration.count());
            }

            double avg_time = 0.0;
            for (double t : times)
            {
                avg_time += t;
            }
            avg_time /= times.size();

            // Create result object
            struct json_object *cpp_shl_const_result_obj = json_object_new_object();
            json_object_object_add(cpp_shl_const_result_obj, "method",
                                   json_object_new_string("CPP_shl_const"));

            struct json_object *output_obj = json_object_new_object();
            if (result.is_top()) {
                json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                json_object_object_add(output_obj, "end", json_object_new_uint64(0));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(64));
                json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(false));
                json_object_object_add(output_obj, "is_top", json_object_new_boolean(true));
            } else {
                json_object_object_add(output_obj, "start", json_object_new_uint64(result.start().get_uint64_t()));
                json_object_object_add(output_obj, "end", json_object_new_uint64(result.end().get_uint64_t()));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(result.get_bitwidth(__LINE__)));
                json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(result.is_bottom()));
                json_object_object_add(output_obj, "is_top", json_object_new_boolean(false));
            }
            json_object_object_add(cpp_shl_const_result_obj, "output", output_obj);
            json_object_object_add(cpp_shl_const_result_obj, "avg_time_ns", json_object_new_double(avg_time));

            struct json_object *new_shl_const_test_case = json_object_new_object();
            json_object_object_add(new_shl_const_test_case, "operation", json_object_new_string(operation));
            json_object_object_add(new_shl_const_test_case, "input_a", input_a_obj);
            json_object_object_add(new_shl_const_test_case, "shift_amount", shift_amount_obj);

            struct json_object *shl_const_test_results_array = json_object_new_array();
            json_object_array_add(shl_const_test_results_array, cpp_shl_const_result_obj);
            json_object_object_add(new_shl_const_test_case, "results", shl_const_test_results_array);

            json_object_array_add(shl_const_results_array, new_shl_const_test_case);
        }
    }

    // Process lshr_const operations
    struct json_object *lshr_const_operations;
    struct json_object *lshr_const_results_array = json_object_new_array();
    if (json_object_object_get_ex(root_obj, "lshr_const_operations", &lshr_const_operations))
    {
        int lshr_const_case_count = json_object_array_length(lshr_const_operations);
        printf("Processing %d lshr_const wrapped interval test cases with C++ implementation...\n", lshr_const_case_count);

        for (int i = 0; i < lshr_const_case_count; i++)
        {
            struct json_object *test_case = json_object_array_get_idx(lshr_const_operations, i);

            // Get operation
            struct json_object *op_obj;
            json_object_object_get_ex(test_case, "operation", &op_obj);
            const char *operation = json_object_get_string(op_obj);

            struct json_object *input_a_obj, *shift_amount_obj;
            json_object_object_get_ex(test_case, "input_a", &input_a_obj);
            json_object_object_get_ex(test_case, "shift_amount", &shift_amount_obj);

            struct json_object *a_start_obj, *a_end_obj, *a_bottom_obj;
            json_object_object_get_ex(input_a_obj, "start", &a_start_obj);
            json_object_object_get_ex(input_a_obj, "end", &a_end_obj);
            json_object_object_get_ex(input_a_obj, "is_bottom", &a_bottom_obj);

            uint64_t a_start = json_object_get_uint64(a_start_obj);
            uint64_t a_end = json_object_get_uint64(a_end_obj);
            bool a_is_bottom = json_object_get_boolean(a_bottom_obj);
            uint64_t shift_amount = json_object_get_uint64(shift_amount_obj);

            crab::wrapint::bitwidth_t width = 64;

            crab::domains::wrapped_interval<DummyNumber> wint_a;
            if (a_is_bottom)
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>::bottom();
            }
            else
            {
                wint_a = crab::domains::wrapped_interval<DummyNumber>(
                    crab::wrapint(a_start, width), crab::wrapint(a_end, width));
            }

            // Perform lshr_const operation and measure time
            std::vector<double> times;
            crab::domains::wrapped_interval<DummyNumber> result;

            for (int iter = 0; iter < iterations; iter++)
            {
                auto start_time = std::chrono::high_resolution_clock::now();
                result = run_cpp_lshr_const_operation(wint_a, shift_amount);
                auto end_time = std::chrono::high_resolution_clock::now();

                auto duration = std::chrono::duration_cast<std::chrono::nanoseconds>(end_time - start_time);
                times.push_back(duration.count());
            }

            double avg_time = 0.0;
            for (double t : times)
            {
                avg_time += t;
            }
            avg_time /= times.size();

            // Create result object
            struct json_object *cpp_lshr_const_result_obj = json_object_new_object();
            json_object_object_add(cpp_lshr_const_result_obj, "method",
                                   json_object_new_string("CPP_lshr_const"));

            struct json_object *output_obj = json_object_new_object();
            if (result.is_top()) {
                json_object_object_add(output_obj, "start", json_object_new_uint64(0));
                json_object_object_add(output_obj, "end", json_object_new_uint64(0));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(64));
                json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(false));
                json_object_object_add(output_obj, "is_top", json_object_new_boolean(true));
            } else {
                json_object_object_add(output_obj, "start", json_object_new_uint64(result.start().get_uint64_t()));
                json_object_object_add(output_obj, "end", json_object_new_uint64(result.end().get_uint64_t()));
                json_object_object_add(output_obj, "bitwidth", json_object_new_int(result.get_bitwidth(__LINE__)));
                json_object_object_add(output_obj, "is_bottom", json_object_new_boolean(result.is_bottom()));
                json_object_object_add(output_obj, "is_top", json_object_new_boolean(false));
            }
            json_object_object_add(cpp_lshr_const_result_obj, "output", output_obj);
            json_object_object_add(cpp_lshr_const_result_obj, "avg_time_ns", json_object_new_double(avg_time));

            struct json_object *new_lshr_const_test_case = json_object_new_object();
            json_object_object_add(new_lshr_const_test_case, "operation", json_object_new_string(operation));
            json_object_object_add(new_lshr_const_test_case, "input_a", input_a_obj);
            json_object_object_add(new_lshr_const_test_case, "shift_amount", shift_amount_obj);

            struct json_object *lshr_const_test_results_array = json_object_new_array();
            json_object_array_add(lshr_const_test_results_array, cpp_lshr_const_result_obj);
            json_object_object_add(new_lshr_const_test_case, "results", lshr_const_test_results_array);

            json_object_array_add(lshr_const_results_array, new_lshr_const_test_case);
        }
    }

    // Create final output structure
    json_object_object_add(output_array, "binary_operations", binary_results_array);
    json_object_object_add(output_array, "at_operations", at_results_array);
    json_object_object_add(output_array, "comparison_operations", comparison_results_array);
    json_object_object_add(output_array, "unary_operations", unary_results_array);

    json_object_object_add(output_array, "shl_const_operations", shl_const_results_array);
    json_object_object_add(output_array, "lshr_const_operations", lshr_const_results_array);

    const char *output_json = json_object_to_json_string_ext(output_array, JSON_C_TO_STRING_PRETTY);
    FILE *out_fp = fopen(output_file, "w");
    if (out_fp)
    {
        fprintf(out_fp, "%s", output_json);
        fclose(out_fp);
        printf("C++ wrapped interval test results saved to: %s\n", output_file);
    }
    else
    {
        perror("Failed to write output file");
    }

    json_object_put(root_obj);
    json_object_put(output_array);
    free(json_str);

    return 0;
}
