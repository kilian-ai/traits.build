#!/usr/bin/env python3
"""Add wasm = true (and special overrides) to trait.toml files."""

def add_wasm_field(filepath, extra_lines=None):
    with open(filepath) as f:
        content = f.read()
    if 'wasm = true' in content:
        print(f'  SKIP (already has wasm): {filepath}')
        return
    lines = content.split('\n')
    new_lines = []
    added = False
    for line in lines:
        new_lines.append(line)
        if not added and line.strip().startswith('entry = '):
            new_lines.append('wasm = true')
            if extra_lines:
                for el in extra_lines:
                    new_lines.append(el)
            added = True
    if not added:
        print(f'  WARN: no entry= found in {filepath}')
    else:
        with open(filepath, 'w') as f:
            f.write('\n'.join(new_lines))
        print(f'  OK: {filepath}')

# Simple wasm = true (no special overrides)
simple_traits = [
    'traits/kernel/call/call.trait.toml',
    'traits/kernel/types/types.trait.toml',
    'traits/sys/call/call.trait.toml',
    'traits/sys/info/info.trait.toml',
    'traits/sys/list/list.trait.toml',
    'traits/sys/llm/llm.trait.toml',
    'traits/sys/openapi/openapi.trait.toml',
    'traits/sys/registry/registry.trait.toml',
    'traits/sys/test_runner/test_runner.trait.toml',
    'traits/sys/version/version.trait.toml',
    'traits/llm/prompt/webllm/webllm.trait.toml',
    'traits/www/admin/admin.trait.toml',
    'traits/www/admin/spa/spa.trait.toml',
    'traits/www/chat_logs/chat_logs.trait.toml',
    'traits/www/docs/docs.trait.toml',
    'traits/www/docs/api/api.trait.toml',
    'traits/www/llm_test/llm_test.trait.toml',
    'traits/www/playground/playground.trait.toml',
    'traits/www/static/static.trait.toml',
    'traits/www/traits/build/build.trait.toml',
    'traits/www/wasm/wasm.trait.toml',
]

for f in simple_traits:
    add_wasm_field(f)

# Special traits with overrides
add_wasm_field('traits/sys/checksum/checksum.trait.toml', 
               ['wasm_entry = "checksum_dispatch"'])

add_wasm_field('traits/sys/cli/wasm/wasm.trait.toml',
               ['wasm_entry = "wasm_dispatch"', 'wasm_source = "wasm_impl.rs"'])

add_wasm_field('traits/sys/ps/wasm/wasm.trait.toml')

add_wasm_field('traits/sys/ps/ps.trait.toml',
               ['wasm_forward = "sys.ps.wasm"', 'helper_preferred = true'])

add_wasm_field('traits/kernel/cli/cli.trait.toml',
               ['wasm_callable = false'])

print('Done!')
