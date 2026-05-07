// hello.c — wasi-sdk + wit-bindgen-c component target.
//
// Build (handled by ../build.sh):
//   1. wit-bindgen c --world hello-world --out-dir . ./wit
//      → emits hello_world.c, hello_world.h, hello_world_component_type.o
//   2. $WASI_SDK/bin/clang -mexec-model=reactor -O2 \
//        -I. hello.c hello_world.c hello_world_component_type.o \
//        -o hello-c.module.wasm
//   3. wasm-tools component new hello-c.module.wasm -o hello-c.component.wasm

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "hello_world.h"

// Avoid stdio (snprintf pulls fd_write which needs wasi imports we don't link).
// Build the JSON greeting by appending into a heap buffer.
static void append(uint8_t **buf, size_t *len, size_t *cap, const char *s, size_t n) {
    if (*len + n > *cap) {
        size_t new_cap = (*cap ? *cap * 2 : 64);
        while (new_cap < *len + n) new_cap *= 2;
        *buf = (uint8_t *)realloc(*buf, new_cap);
        *cap = new_cap;
    }
    memcpy(*buf + *len, s, n);
    *len += n;
}

bool exports_hello_c_hello_hello(hello_world_string_t *name,
                                 hello_world_string_t *ret,
                                 hello_world_string_t *err) {
    (void)err;

    static const char prefix[] = "{\"greeting\":\"Hello, ";
    static const char suffix[] = "!\",\"language\":\"C\",\"toolchain\":\"wasi-sdk + wit-bindgen-c\"}";

    uint8_t *buf = NULL;
    size_t len = 0, cap = 0;

    append(&buf, &len, &cap, prefix, sizeof(prefix) - 1);
    for (size_t i = 0; i < name->len; i++) {
        char c = (char)name->ptr[i];
        if (c == '\\' || c == '"') append(&buf, &len, &cap, "\\", 1);
        append(&buf, &len, &cap, &c, 1);
    }
    append(&buf, &len, &cap, suffix, sizeof(suffix) - 1);

    ret->ptr = buf;
    ret->len = len;
    return true;
}
