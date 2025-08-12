#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <stdbool.h>
#include <unistd.h>
#include <json-c/json.h>

// Crab/CLAM dependencies
#include "crab/numbers/wrapint.hpp"
#include "crab/domains/tnum.hpp"
#include "crab/domains/tnum_impl.hpp"

// Dummy implementation for ikos::error
namespace ikos {
void error(const char *msg) {
    fprintf(stderr, "ikos error: %s\n", msg);
    exit(1);
}
}

// Dummy implementation for crab::CrabStats
namespace crab {

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

// Global instance
// We don't have access to the definition of these static members so we cannot
// have them.
// std::map<std::string, unsigned> &CrabStats::getCounters() {
//   static std::map<std::string, unsigned> counters;
//   return counters;
// }
// std::map<std::string, Stopwatch> &CrabStats::getTimers() {
//   static std::map<std::string, Stopwatch> timers;
//   return timers;
// }

} // namespace crab

// Dummy class for the Number template parameter
class DummyNumber {};

int main(int argc, char *argv[]) {
    if (argc < 2) {
        printf("Usage: %s <rust_test_cases.json>\n", argv[0]);
        return 1;
    }

    const char *input_file = argv[1];
    const char *output_file = "./tests/build/cpp_test_results.json";
    const int iterations = (argc > 2) ? atoi(argv[2]) : 1000;

    FILE *fp = fopen(input_file, "r");
    if (!fp) {
        perror("Failed to open input file");
        return 1;
    }

    fseek(fp, 0, SEEK_END);
    long file_size = ftell(fp);
    fseek(fp, 0, SEEK_SET);

    char *json_str = (char *)malloc(file_size + 1);
    if (!json_str) {
        perror("Failed to allocate memory for json file");
        fclose(fp);
        return 1;
    }
    if (fread(json_str, 1, file_size, fp) != (size_t)file_size) {
        fprintf(stderr, "Warning: Did not read the full file content.\n");
    }
    json_str[file_size] = '\0';
    fclose(fp);

    struct json_object *root_obj = json_tokener_parse(json_str);
    if (!root_obj) {
        fprintf(stderr, "Failed to parse JSON\n");
        free(json_str);
        return 1;
    }

    struct json_object *output_array = json_object_new_array();
    int case_count = json_object_array_length(root_obj);
    printf("Processing %d test cases with C++ implementation...\n", case_count);

    for (int i = 0; i < case_count; i++) {
        struct json_object *test_case = json_object_array_get_idx(root_obj, i);

        // Get operation
        struct json_object *op_obj;
        json_object_object_get_ex(test_case, "operation", &op_obj);
        const char *operation = json_object_get_string(op_obj);

        struct json_object *input_a_obj, *input_b_obj;
        json_object_object_get_ex(test_case, "input_a", &input_a_obj);
        json_object_object_get_ex(test_case, "input_b", &input_b_obj);

        uint64_t a_value = json_object_get_uint64(json_object_object_get(input_a_obj, "value"));
        uint64_t a_mask = json_object_get_uint64(json_object_object_get(input_a_obj, "mask"));
        uint64_t b_value = json_object_get_uint64(json_object_object_get(input_b_obj, "value"));
        uint64_t b_mask = json_object_get_uint64(json_object_object_get(input_b_obj, "mask"));
        
        crab::wrapint::bitwidth_t width = 64;
        crab::domains::tnum<DummyNumber> tnum_a(crab::wrapint(a_value, width), crab::wrapint(a_mask, width));
        crab::domains::tnum<DummyNumber> tnum_b(crab::wrapint(b_value, width), crab::wrapint(b_mask, width));

        struct json_object *results_obj;
        json_object_object_get_ex(test_case, "results", &results_obj);
        
        if (tnum_a.is_bottom() || tnum_b.is_bottom()) {
            json_object_array_add(output_array, json_object_get(test_case));
            continue;
        }

        crab::domains::tnum<DummyNumber> cpp_result;
        struct timespec start, end;
        
        clock_gettime(CLOCK_MONOTONIC, &start);
        for (int j = 0; j < iterations; j++) {
            if (strcmp(operation, "add") == 0) {
                cpp_result = tnum_a + tnum_b;
            } else if (strcmp(operation, "sub") == 0) {
                cpp_result = tnum_a - tnum_b;
            } else if (strcmp(operation, "mul") == 0) {
                cpp_result = tnum_a * tnum_b;
            } else if (strcmp(operation, "sdiv") == 0) {
                cpp_result = tnum_a / tnum_b;
            } else if (strcmp(operation, "udiv") == 0) {
                cpp_result = tnum_a.UDiv(tnum_b);
            } else if (strcmp(operation, "srem") == 0) {
                cpp_result = tnum_a.SRem(tnum_b);
            } else if (strcmp(operation, "urem") == 0) {
                cpp_result = tnum_a.URem(tnum_b);
            } else if (strcmp(operation, "and") == 0) {
                cpp_result = tnum_a & tnum_b;
            } else if (strcmp(operation, "or") == 0) {
                cpp_result = tnum_a | tnum_b;
            } else if (strcmp(operation, "xor") == 0) {
                cpp_result = tnum_a.Xor(tnum_b); 
            } else if (strcmp(operation, "not") == 0) {
                cpp_result = ~tnum_a; 
            } else if (strcmp(operation, "lshr_const") == 0) {
                cpp_result = tnum_a.LShr(tnum_b.value().get_uint64_t());
            } else if (strcmp(operation, "shl_const") == 0) {
                cpp_result = tnum_a.Shl(tnum_b.value().get_uint64_t());
            } else if (strcmp(operation, "ashr_const") == 0) {
                cpp_result = tnum_a.AShr(tnum_b.value().get_uint64_t());
            } else if (strcmp(operation, "lshr") == 0) {
                cpp_result = tnum_a.LShr(tnum_b);
            } else if (strcmp(operation, "shl") == 0) {
                cpp_result = tnum_a.Shl(tnum_b);
            } else if (strcmp(operation, "eq") == 0) {
                cpp_result = crab::domains::tnum<DummyNumber>(crab::wrapint((tnum_a == tnum_b ? 1 : 0), width), crab::wrapint(0, width));
            } else if (strcmp(operation, "ne") == 0) {
                cpp_result = crab::domains::tnum<DummyNumber>(crab::wrapint((tnum_a != tnum_b ? 1 : 0), width), crab::wrapint(0, width));
            } else if (strcmp(operation, "gt") == 0) {
                cpp_result = crab::domains::tnum<DummyNumber>(crab::wrapint((tnum_a.is_nonnegative() && tnum_b.is_negative() ? 1 : 0), width), crab::wrapint(0, width));
            } else if (strcmp(operation, "ge") == 0) {
                cpp_result = crab::domains::tnum<DummyNumber>(crab::wrapint((!tnum_a.is_negative() || tnum_b.is_negative() ? 1 : 0), width), crab::wrapint(0, width));
            } else if (strcmp(operation, "lt") == 0) {
                cpp_result = crab::domains::tnum<DummyNumber>(crab::wrapint((tnum_a.is_negative() && !tnum_b.is_negative() ? 1 : 0), width), crab::wrapint(0, width));
            } else if (strcmp(operation, "le") == 0) {
                cpp_result = crab::domains::tnum<DummyNumber>(crab::wrapint((tnum_a.is_negative() || !tnum_b.is_negative() ? 1 : 0), width), crab::wrapint(0, width));
            }
        }
        clock_gettime(CLOCK_MONOTONIC, &end);
        
        double time_taken_ns = ((double)(end.tv_sec - start.tv_sec) * 1e9 + (end.tv_nsec - start.tv_nsec)) / iterations;

        char method_name[128];
        snprintf(method_name, sizeof(method_name), "CPP_%s", operation);

        struct json_object *cpp_result_obj = json_object_new_object();
        json_object_object_add(cpp_result_obj, "method", json_object_new_string(method_name));
        
        struct json_object *cpp_output_obj = json_object_new_object();

        if (cpp_result.is_top()) {
            // Top: 完全不确定 - value = 0, mask = 0xFFFFFFFFFFFFFFFF
            json_object_object_add(cpp_output_obj, "value", json_object_new_uint64(0));
            json_object_object_add(cpp_output_obj, "mask", json_object_new_uint64(0xFFFFFFFFFFFFFFFF));
        } else if (cpp_result.is_bottom()) {
            // Bottom: 不可能的值 - value = 0xFFFFFFFFFFFFFFFF, mask = 0xFFFFFFFFFFFFFFFF
            json_object_object_add(cpp_output_obj, "value", json_object_new_uint64(0xFFFFFFFFFFFFFFFF));
            json_object_object_add(cpp_output_obj, "mask", json_object_new_uint64(0xFFFFFFFFFFFFFFFF));
        } else {
            // 正常情况：输出实际的 value 和 mask
            json_object_object_add(cpp_output_obj, "value", json_object_new_uint64(cpp_result.value().get_uint64_t()));
            json_object_object_add(cpp_output_obj, "mask", json_object_new_uint64(cpp_result.mask().get_uint64_t()));
        }
        
        json_object_object_add(cpp_result_obj, "output", cpp_output_obj);
        json_object_object_add(cpp_result_obj, "avg_time_ns", json_object_new_double(time_taken_ns));

        json_object_array_add(results_obj, cpp_result_obj);
        json_object_array_add(output_array, json_object_get(test_case));
    }

    const char *output_json = json_object_to_json_string_ext(output_array, JSON_C_TO_STRING_PRETTY);
    FILE *out_fp = fopen(output_file, "w");
    if (out_fp) {
        fputs(output_json, out_fp);
        fclose(out_fp);
        printf("\nC++ results saved to: %s\n", output_file);
    } else {
        perror("Failed to open output file for C++ results");
    }

    json_object_put(root_obj);
    json_object_put(output_array);
    free(json_str);

    return 0;
}