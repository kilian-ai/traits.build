// jco componentize bundles this file into a component implementing the
// `hello` interface from wit/hello.wit. The export name matches the interface.
// componentize-js maps wit `result<string, string>` to: return string on ok,
// throw on err. Do NOT return a {tag, val} object.
export const hello = {
    hello(name) {
        const safe = String(name).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
        return `{"greeting":"Hello, ${safe}!","language":"JavaScript","toolchain":"jco"}`;
    },
};
