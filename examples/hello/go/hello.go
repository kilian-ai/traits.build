// hello.go — TinyGo + wit-bindgen-go component target.
//
// Build steps (handled by ../build.sh):
//   1. wit-bindgen-go generate --world hello-world --out internal/ ./wit
//   2. tinygo build -target=wasip2 -o hello-go.module.wasm .
//   3. wasm-tools component embed --world hello-world ./wit hello-go.module.wasm -o hello-go.embed.wasm
//   4. wasm-tools component new hello-go.embed.wasm -o hello-go.component.wasm
package main

import (
	"fmt"

	hello "hello-go/internal/hello/go/hello"
	"go.bytecodealliance.org/cm"
)

func init() {
	hello.Exports.Hello = func(name string) cm.Result[string, string, string] {
		out := fmt.Sprintf(
			`{"greeting":"Hello, %s!","language":"Go","toolchain":"TinyGo + wit-bindgen-go"}`,
			escapeJSON(name),
		)
		return cm.OK[cm.Result[string, string, string]](out)
	}
}

// main is required for the entrypoint, but is never called.
func main() {}

func escapeJSON(s string) string {
	out := make([]byte, 0, len(s))
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c == '\\' || c == '"' {
			out = append(out, '\\')
		}
		out = append(out, c)
	}
	return string(out)
}
