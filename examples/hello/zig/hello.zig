// hello.zig — Zig wasi target + wit-bindgen-c bindings.
//
// Zig has no native WIT generator yet, but it links cleanly against the C
// bindings emitted by `wit-bindgen c`. Build steps (handled by ../build.sh):
//   1. wit-bindgen c --world hello-world ./wit -o .
//   2. zig build-lib hello.zig hello_world.c \
//          -target wasm32-wasi -dynamic -rdynamic \
//          -fno-entry --export=exports_hello_zig_hello_hello \
//          -OReleaseSmall -o hello-zig.module.wasm
//   3. wasm-tools component embed --world hello-world ./wit \
//          hello-zig.module.wasm -o hello-zig.embed.wasm
//   4. wasm-tools component new hello-zig.embed.wasm -o hello-zig.component.wasm

const std = @import("std");

const hello_world_string_t = extern struct {
    ptr: [*]u8,
    len: usize,
};

export fn exports_hello_zig_hello_hello(
    name: *hello_world_string_t,
    ret: *hello_world_string_t,
    err: *hello_world_string_t,
) bool {
    _ = err;

    const allocator = std.heap.wasm_allocator;
    const slice = name.ptr[0..name.len];

    const prefix = "{\"greeting\":\"Hello, ";
    const suffix = "!\",\"language\":\"Zig\",\"toolchain\":\"zig + wit-bindgen-c\"}";

    // Worst case: every input byte needs a `\` escape.
    const cap = prefix.len + slice.len * 2 + suffix.len;
    const buf = allocator.alloc(u8, cap) catch return false;

    var n: usize = 0;
    @memcpy(buf[n .. n + prefix.len], prefix);
    n += prefix.len;
    for (slice) |c| {
        if (c == '\\' or c == '"') {
            buf[n] = '\\';
            n += 1;
        }
        buf[n] = c;
        n += 1;
    }
    @memcpy(buf[n .. n + suffix.len], suffix);
    n += suffix.len;

    ret.ptr = buf.ptr;
    ret.len = n;
    return true;
}
