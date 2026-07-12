/* wasm2c harness for the file-organizer planning core.
 *
 * The Rust core is compiled to wasm32-unknown-unknown (no WASI) and translated
 * to C by wasm2c. That module has NO imports: it exports its own linear memory
 * plus three functions (bench_alloc, bench_run). This harness owns all I/O:
 * it reads the directory listing from stdin, hands it to the module through
 * linear memory, runs the planner, and writes the report to stdout.
 *
 * Identical source is compiled by both clang (native ELF) and cosmocc
 * (Actually-Portable Executable) — the two wasm2c endpoints.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bench.h"

#define M w2c_file__organizer__bench

static unsigned char *read_all_stdin(size_t *out_len) {
  size_t cap = 1 << 20, len = 0;
  unsigned char *buf = (unsigned char *)malloc(cap);
  if (!buf) {
    perror("malloc");
    exit(1);
  }
  for (;;) {
    if (len == cap) {
      cap *= 2;
      buf = (unsigned char *)realloc(buf, cap);
      if (!buf) {
        perror("realloc");
        exit(1);
      }
    }
    size_t n = fread(buf + len, 1, cap - len, stdin);
    len += n;
    if (n == 0)
      break;
  }
  *out_len = len;
  return buf;
}

int main(void) {
  size_t len = 0;
  unsigned char *input = read_all_stdin(&len);

  wasm_rt_init();
  M inst;
  wasm2c_file__organizer__bench_instantiate(&inst);
  wasm_rt_memory_t *mem = w2c_file__organizer__bench_memory(&inst);

  /* A 4-byte slot for the output length, then the input buffer. Allocations
     may grow (and thus reallocate) linear memory, so mem->data is only read
     back after all allocation/run calls have returned. */
  u32 out_len_ptr = w2c_file__organizer__bench_bench_alloc(&inst, 4);
  u32 in_ptr = w2c_file__organizer__bench_bench_alloc(&inst, (u32)len);
  memcpy(mem->data + in_ptr, input, len);

  u32 out_ptr =
      w2c_file__organizer__bench_bench_run(&inst, in_ptr, (u32)len, out_len_ptr);

  u32 out_len;
  memcpy(&out_len, mem->data + out_len_ptr, sizeof(out_len));
  fwrite(mem->data + out_ptr, 1, out_len, stdout);
  fputc('\n', stdout);

  wasm2c_file__organizer__bench_free(&inst);
  wasm_rt_free();
  free(input);
  return 0;
}
