;; hello.scm — TinyScheme greeting implementation.
;; Loaded into a fresh scheme context by bridge.c, which then calls
;; (hello name) and ships the returned string back through the
;; wit-bindgen-c export.

(define (hello name)
  (string-append
    "{\"greeting\":\"Hello, " name
    "!\",\"language\":\"Scheme\""
    ",\"toolchain\":\"wasi-sdk + TinyScheme + wit-bindgen-c\"}"))
