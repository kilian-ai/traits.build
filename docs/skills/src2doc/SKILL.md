---
name: src2doc
description: |
  You are generating a **1:1 mapped documentation file** for a single source code file.

  Your goal is to produce a Markdown document that mirrors the code’s structure, naming, and behavior precisely.
---

## Rules

1. **One file → one document**
2. **Use exact names from code (functions, variables, modules)**
3. **Do not invent behavior**
4. **Do not repeat code line-by-line**
5. **Focus on behavior, state, and interactions**
6. **Prefer concrete examples over types**
7. **Be exhaustive: no hidden logic**
8. **If something is unclear, infer minimally and mark with `?`**

---

## Required Output Format (STRICT)

Return ONLY Markdown using this exact structure:

# <file/module name>

## Purpose

Clear, concise description of what this module does.

## Exports

List all exported functions/classes/constants with short descriptions.

---

## <function or class name>

### Purpose

What it does (behavior, not implementation).

### Inputs

* paramName: description (values, not types unless critical)

### Outputs

* return value description

### State

reads:

* ...
  writes:
* ...

### Side Effects

* external effects (I/O, network, mutation, etc.)

### Dependencies

* internal calls
* external modules

### Flow

Step-by-step behavior in order.

### Edge Cases

* important edge conditions

### Example

Concrete usage with real values.

---

(repeat for every function/class)

---

## Internal Structure

Describe how parts relate inside this file.

## Notes

Optional clarifications or uncertainties (`?` if inferred).

---

## Style Constraints

* Use simple, direct language
* No filler text
* No repetition
* No explanations about the task
* No code blocks unless in Examples
* Keep sections even if empty (write “none”)

---

## Input

The following is the source code file to document:

or it is attached


IF an MD file exists with the same name and its newer than the source file then do the following:

You are generating or updating (if it is added below) a **source code file** from a Markdown documentation file.

The documentation is a **complete behavioral specification**. Your job is to reconstruct the code so that it matches the documented behavior exactly.

---

## Rules

1. **Docs are the source of truth**
2. **Do not invent behavior beyond docs**
3. **Preserve all names exactly (functions, variables, modules)**
4. **Implement all flows deterministically**
5. **State transitions must match exactly**
6. **If ambiguity exists, choose the simplest implementation and mark with `TODO`**
7. **No extra features, abstractions, or optimizations**
8. **Match examples exactly (they must run correctly)**

---

## Mapping Strategy

### Functions

Each `## <function>` section → one function

### Inputs / Outputs

* Use names exactly
* Types optional unless clearly implied

### State

Translate explicitly:

reads → inputs, selectors, or getters
writes → mutations, returns, or side effects

---

### Flow → Code

Convert step-by-step into actual control flow:

* ordered steps → sequential code
* conditions → if/else
* loops → loops
* branching must match exactly

---

### Dependencies

* Internal → function calls
* External → imports

---

### Side Effects

Implement explicitly:

* I/O
* network
* storage
* mutations

---

### Edge Cases

Must be handled explicitly in code.

---

### Examples

Examples are **tests**:

* ensure they execute correctly
* use them to validate logic

---

## Output Format

* Return ONLY code
* No explanations
* No markdown
* No comments unless necessary
* Use a single file

ALWAYS CREATE THE MD FILE IN THE SAME FOLDER AS THE SOURCE FILE, WITH THE SAME NAME BUT `.md` EXTENSION.

Then create a `.features.json` file out of `.md` including tests for all features.

---

## features.json Format

Create a `<basename>.features.json` in the same folder as the source file. Structure:

```json
{
  "module": "src/example.rs",
  "description": "Short module description",
  "features": [
    {
      "name": "Feature name",
      "description": "What this feature does",
      "tests": [
        {
          "name": "descriptive test name",
          "type": "command",
          "command": "./target/release/traits call sys.example arg1 2>/dev/null",
          "checks": [
            { "type": "exit_code", "expected": 0 },
            { "type": "contains", "expected": "expected substring in stdout or stderr" },
            { "type": "not_contains", "expected": "should not appear" },
            { "type": "count_gte", "expected": 5 }
          ]
        }
      ]
    }
  ]
}
```

### Check types

| type | expected | behavior |
|------|----------|----------|
| `exit_code` | integer | assert command exit code equals expected |
| `contains` | string | assert stdout OR stderr contains expected substring |
| `not_contains` | string | assert neither stdout nor stderr contains expected substring |
| `count_gte` | integer | parse stdout as integer, assert >= expected |

### Rules

* One test per distinct behavior — name describes the assertion
* Use `./target/release/traits` prefix for all CLI commands
* Redirect stderr with `2>/dev/null` when only testing stdout, `2>&1` when testing stderr
* Every feature must have at least one test
* Every exported function/behavior from the `.md` should map to at least one feature

Then create a .features.json file out of .md including tests for all features