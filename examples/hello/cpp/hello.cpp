// hello.cpp — wasi-sdk + wit-bindgen-c (callable from C++) component target.
//
// wit-bindgen has no first-class C++ generator, so we use the C bindings and
// link them against this C++ TU. We deliberately stay free of STL containers
// that pull exception machinery (string, vector) — wasi-sdk's libc++ would
// otherwise drag in __cxa_allocate_exception which has no host import.
//
// Build flags (set in ../build.sh): -fno-exceptions -fno-rtti.

#include <cstdint>
#include <cstdlib>
#include <cstring>

extern "C" {
#include "hello_world.h"
}

namespace {

void append(uint8_t **buf, std::size_t *len, std::size_t *cap,
            const char *s, std::size_t n) {
    if (*len + n > *cap) {
        std::size_t new_cap = (*cap ? *cap * 2 : 64);
        while (new_cap < *len + n) new_cap *= 2;
        *buf = static_cast<uint8_t *>(std::realloc(*buf, new_cap));
        *cap = new_cap;
    }
    std::memcpy(*buf + *len, s, n);
    *len += n;
}

}  // namespace

extern "C" bool exports_hello_cpp_hello_hello(hello_world_string_t *name,
                                              hello_world_string_t *ret,
                                              hello_world_string_t *err) {
    (void)err;
    static const char prefix[] = "{\"greeting\":\"Hello, ";
    static const char suffix[] = "!\",\"language\":\"C++\",\"toolchain\":\"wasi-sdk + wit-bindgen-c (C++ TU)\"}";

    uint8_t *buf = nullptr;
    std::size_t len = 0, cap = 0;
    append(&buf, &len, &cap, prefix, sizeof(prefix) - 1);
    for (std::size_t i = 0; i < name->len; i++) {
        char c = static_cast<char>(name->ptr[i]);
        if (c == '\\' || c == '"') append(&buf, &len, &cap, "\\", 1);
        append(&buf, &len, &cap, &c, 1);
    }
    append(&buf, &len, &cap, suffix, sizeof(suffix) - 1);

    ret->ptr = buf;
    ret->len = len;
    return true;
}
