/* Universal file-organizer driver.
 *
 * A dumb filesystem shell around the verified Rust reactor (compiled to
 * WebAssembly, translated to C by wasm2c). It does exactly two things the
 * reactor cannot: observe the directory, and replay the primitive ops the
 * reactor returns. It contains ZERO organizing logic — every decision, the
 * report text, the manifest JSON and exit codes come from the reactor.
 *
 * Built with cosmocc -> one Actually-Portable Executable for Win/macOS/Linux.
 */
#include <dirent.h>
#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#include "reactor.h"

#define PATHCAP 8192

/* ---- growable byte buffer ---- */
typedef struct {
  char *d;
  size_t len, cap;
} Buf;
static void buf_reserve(Buf *b, size_t extra) {
  if (b->len + extra > b->cap) {
    b->cap = (b->len + extra) * 2 + 64;
    b->d = (char *)realloc(b->d, b->cap);
    if (!b->d) {
      perror("realloc");
      exit(3);
    }
  }
}
static void buf_add(Buf *b, const void *s, size_t n) {
  buf_reserve(b, n);
  memcpy(b->d + b->len, s, n);
  b->len += n;
}
static void buf_printf(Buf *b, const char *fmt, ...) {
  va_list ap, ap2;
  va_start(ap, fmt);
  va_copy(ap2, ap);
  int n = vsnprintf(NULL, 0, fmt, ap);
  va_end(ap);
  if (n < 0) {
    va_end(ap2);
    return;
  }
  buf_reserve(b, (size_t)n + 1);
  vsnprintf(b->d + b->len, (size_t)n + 1, fmt, ap2);
  va_end(ap2);
  b->len += (size_t)n;
}

/* ---- recursive snapshot: append "K\trel\n" lines ---- */
static void snap(const char *absdir, const char *rel, Buf *out, int *count) {
  DIR *d = opendir(absdir);
  if (!d) return;
  struct dirent *e;
  while ((e = readdir(d))) {
    if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
    char cabs[PATHCAP], crel[PATHCAP];
    snprintf(cabs, sizeof cabs, "%s/%s", absdir, e->d_name);
    if (rel[0])
      snprintf(crel, sizeof crel, "%s/%s", rel, e->d_name);
    else
      snprintf(crel, sizeof crel, "%s", e->d_name);
    struct stat st;
    char kind = 'O';
    if (lstat(cabs, &st) == 0) {
      if (S_ISLNK(st.st_mode)) kind = 'L';
      else if (S_ISDIR(st.st_mode)) kind = 'D';
      else if (S_ISREG(st.st_mode)) kind = 'F';
    }
    buf_printf(out, "%c\t%s\n", kind, crel);
    (*count)++;
    if (kind == 'D') snap(cabs, crel, out, count);
  }
  closedir(d);
}

/* ---- op execution helpers ---- */
static void mkdir_p(const char *base, const char *rel) {
  char path[PATHCAP];
  int n = snprintf(path, sizeof path, "%s/%s", base, rel);
  if (n < 0 || n >= (int)sizeof path) return;
  size_t start = strlen(base) + 1; /* don't try to mkdir the base itself */
  for (size_t i = start; path[i]; i++) {
    if (path[i] == '/') {
      path[i] = '\0';
      mkdir(path, 0755);
      path[i] = '/';
    }
  }
  mkdir(path, 0755);
}
static void mkdir_parent_of(const char *base, const char *rel) {
  char tmp[PATHCAP];
  snprintf(tmp, sizeof tmp, "%s", rel);
  char *slash = strrchr(tmp, '/');
  if (slash) {
    *slash = '\0';
    mkdir_p(base, tmp);
  }
}
static int move_file(const char *src, const char *dst) {
  if (rename(src, dst) == 0) return 0;
  /* cross-device fallback: copy then unlink */
  FILE *in = fopen(src, "rb");
  if (!in) return -1;
  FILE *out = fopen(dst, "wb");
  if (!out) {
    fclose(in);
    return -1;
  }
  char buf[65536];
  size_t r;
  while ((r = fread(buf, 1, sizeof buf, in)) > 0) fwrite(buf, 1, r, out);
  fclose(in);
  fclose(out);
  return remove(src);
}

/* ---- output cursor (tokenize in place) ---- */
typedef struct {
  char *p, *end;
} Cur;
static char *cur_line(Cur *c) {
  char *s = c->p;
  while (c->p < c->end && *c->p != '\n') c->p++;
  if (c->p < c->end) {
    *c->p = '\0';
    c->p++;
  }
  return s;
}
static char *cur_take(Cur *c, size_t n) {
  char *s = c->p;
  if (c->p + n <= c->end) c->p += n;
  else c->p = c->end;
  return s;
}

int main(int argc, char **argv) {
  /* find the folder (first non-flag arg) so we can snapshot it up front */
  const char *folder_arg = NULL;
  for (int i = 1; i < argc; i++) {
    if (argv[i][0] != '-') {
      folder_arg = argv[i];
      break;
    }
  }

  int exists = 0, is_dir = 0;
  char base[PATHCAP] = {0};
  const char *folder_display = folder_arg ? folder_arg : "";
  Buf entries = {0};
  int entry_count = 0;
  Buf manifest = {0};
  int have_manifest = 0;

  if (folder_arg) {
    struct stat st;
    exists = stat(folder_arg, &st) == 0;
    is_dir = exists && S_ISDIR(st.st_mode);
    if (is_dir) {
      char *rp = realpath(folder_arg, NULL);
      if (rp) {
        snprintf(base, sizeof base, "%s", rp);
        free(rp);
      } else {
        snprintf(base, sizeof base, "%s", folder_arg);
      }
      folder_display = base;
      snap(base, "", &entries, &entry_count);
      char mpath[PATHCAP];
      snprintf(mpath, sizeof mpath, "%s/.file_organizer_manifest.json", base);
      FILE *mf = fopen(mpath, "rb");
      if (mf) {
        have_manifest = 1;
        char tmp[65536];
        size_t r;
        while ((r = fread(tmp, 1, sizeof tmp, mf)) > 0) buf_add(&manifest, tmp, r);
        fclose(mf);
      }
    }
  }

  /* now (UTC) for the manifest timestamp */
  char now[64] = "1970-01-01T00:00:00+00:00";
  time_t t = time(NULL);
  struct tm tmv;
  if (gmtime_r(&t, &tmv))
    strftime(now, sizeof now, "%Y-%m-%dT%H:%M:%S+00:00", &tmv);

  /* ---- build the framed input ---- */
  Buf in = {0};
  buf_printf(&in, "ARGS %d\n", argc - 1);
  for (int i = 1; i < argc; i++) buf_printf(&in, "%s\n", argv[i]);
  buf_printf(&in, "EXISTS %d\n", exists ? 1 : 0);
  buf_printf(&in, "ISDIR %d\n", is_dir ? 1 : 0);
  buf_printf(&in, "NOW %s\n", now);
  buf_printf(&in, "FOLDER %s\n", folder_display);
  buf_printf(&in, "ENTRIES %d\n", entry_count);
  buf_add(&in, entries.d ? entries.d : "", entries.len);
  buf_printf(&in, "MANIFEST %d\n", have_manifest ? (int)manifest.len : -1);
  if (have_manifest) buf_add(&in, manifest.d, manifest.len);

  /* ---- run the reactor (wasm2c) ---- */
  wasm_rt_init();
  w2c_reactor inst;
  wasm2c_reactor_instantiate(&inst);
  wasm_rt_memory_t *mem = w2c_reactor_memory(&inst);
  u32 outlen_ptr = w2c_reactor_reactor_alloc(&inst, 4);
  u32 in_ptr = w2c_reactor_reactor_alloc(&inst, (u32)in.len);
  memcpy(mem->data + in_ptr, in.d, in.len);
  u32 out_ptr = w2c_reactor_reactor_run(&inst, in_ptr, (u32)in.len, outlen_ptr);
  u32 outlen;
  memcpy(&outlen, mem->data + outlen_ptr, sizeof outlen);
  char *outbuf = (char *)malloc((size_t)outlen + 1);
  memcpy(outbuf, mem->data + out_ptr, outlen);
  outbuf[outlen] = '\0';
  wasm2c_reactor_free(&inst);
  wasm_rt_free();

  /* ---- parse the framed output ---- */
  Cur c = {outbuf, outbuf + outlen};
  int exit_code = 0;
  sscanf(cur_line(&c), "EXIT %d", &exit_code);

  size_t solen = 0;
  sscanf(cur_line(&c), "STDOUT %zu", &solen);
  char *sob = cur_take(&c, solen);
  fwrite(sob, 1, solen, stdout);

  size_t selen = 0;
  sscanf(cur_line(&c), "STDERR %zu", &selen);
  char *seb = cur_take(&c, selen);
  fwrite(seb, 1, selen, stderr);

  int nops = 0;
  sscanf(cur_line(&c), "OPS %d", &nops);
  for (int i = 0; i < nops; i++) {
    char *line = cur_line(&c);
    char *tab = strchr(line, '\t');
    if (!tab) continue;
    *tab = '\0';
    char *rest = tab + 1;
    if (base[0] == '\0') continue; /* no target dir: nothing to execute */

    if (!strcmp(line, "MKDIR")) {
      mkdir_p(base, rest);
    } else if (!strcmp(line, "RMDIR")) {
      char p[PATHCAP];
      snprintf(p, sizeof p, "%s/%s", base, rest);
      rmdir(p);
    } else if (!strcmp(line, "DELETE")) {
      char p[PATHCAP];
      snprintf(p, sizeof p, "%s/%s", base, rest);
      remove(p);
    } else if (!strcmp(line, "MOVE")) {
      char *t2 = strchr(rest, '\t');
      if (!t2) continue;
      *t2 = '\0';
      char *to = t2 + 1;
      char src[PATHCAP], dst[PATHCAP];
      snprintf(src, sizeof src, "%s/%s", base, rest);
      snprintf(dst, sizeof dst, "%s/%s", base, to);
      mkdir_parent_of(base, to);
      move_file(src, dst);
    } else if (!strcmp(line, "WRITE")) {
      char *t2 = strchr(rest, '\t');
      if (!t2) continue;
      *t2 = '\0';
      char *path = rest;
      size_t wlen = strtoul(t2 + 1, NULL, 10);
      char *content = cur_take(&c, wlen);
      char p[PATHCAP];
      snprintf(p, sizeof p, "%s/%s", base, path);
      FILE *wf = fopen(p, "wb");
      if (wf) {
        fwrite(content, 1, wlen, wf);
        fclose(wf);
      }
    }
  }

  free(outbuf);
  return exit_code;
}
