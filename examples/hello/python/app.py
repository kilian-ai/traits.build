"""hello.python — componentize-py target.

Build:
    componentize-py -d wit -w hello-world componentize app -o hello-python.component.wasm

componentize-py generates Python bindings under `./hello_world/` from the WIT,
and our `hello` function is picked up by the export name `hello`.
"""

# componentize-py emits a module that we import by world-name. We define a class
# with the same shape as the generated `Hello` protocol stub.

import json


class Hello:
    def hello(self, name: str) -> str:
        # `result<string, string>` — raising propagates as Err; returning is Ok.
        payload = {
            "greeting": f"Hello, {name}!",
            "language": "Python",
            "toolchain": "componentize-py",
        }
        return json.dumps(payload)
