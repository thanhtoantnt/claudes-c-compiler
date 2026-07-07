# Property Ledger: `src/frontend/preprocessor/conditionals.rs`

## `eval_const_expr` preserves truth value under redundant outer parentheses and whitespace
- Tier: 3
- Rationale: The README documents `eval_const_expr` as a recursive-descent parser for C preprocessor constant expressions, and the implementation trims whitespace before parsing. Wrapping a valid expression in extra parentheses and whitespace should not change the truth value. Stronger considered: state machine (rejected — `eval_const_expr` is pure and has no lifecycle/state), differential (rejected — no independent evaluator implementation). Evidence: `src/frontend/preprocessor/README.md:277-294`, `src/frontend/preprocessor/conditionals.rs:246-259`.
- Test file: `src/frontend/preprocessor/conditionals.rs`
- Status: passing
- Counterexample: (none)

```property
function: frontend::preprocessor::eval_const_expr
oracle: algebraic.metamorphic
predicate:
  quantifier: forall
  vars: [expr]
  body: 'eval_const_expr(&format!(" \\t(  {}  )\\n", expr)) == eval_const_expr(&expr)'
generators:
  expr:
    gen: recursive
    depth: 3
    maxSize: 96
    branchFactor: 16
    base:
      gen: oneof
      options:
        - { gen: const, value: "0" }
        - { gen: const, value: "1" }
        - { gen: const, value: "42" }
        - { gen: const, value: "0U" }
        - { gen: const, value: "0x10" }
        - { gen: const, value: "0x8000000000000000" }
        - { gen: const, value: "07" }
        - { gen: const, value: "3ULL" }
        - { gen: const, value: "'a'" }
        - { gen: const, value: "'\\n'" }
        - { gen: const, value: "'\\0'" }
        - { gen: const, value: "true" }
        - { gen: const, value: "false" }
evidence: src/frontend/preprocessor/README.md:277-294, src/frontend/preprocessor/conditionals.rs:250-259
```

## `eval_const_expr` treats bare identifiers as false
- Tier: 4
- Rationale: The implementation's `parse_primary` maps undefined identifiers to `0`, except for the two special literals `true` and `false`. This is a direct negative/reference contract from the code and README, and it is stronger than a crash-only check because the output is fully specified. Stronger considered: state machine (rejected — pure function), differential (rejected — no second implementation). Evidence: `src/frontend/preprocessor/README.md:271-275`, `src/frontend/preprocessor/conditionals.rs:795-804`.
- Test file: `src/frontend/preprocessor/conditionals.rs`
- Status: passing
- Counterexample: (none)

```property
function: frontend::preprocessor::eval_const_expr
oracle: reference
predicate:
  quantifier: forall
  vars: [ident]
  body: '!eval_const_expr(&ident)'
generators:
  ident:
    gen: string
    minLen: 1
    maxLen: 8
    alphabet: [a-zA-Z0-9_]
    first: [a-zA-Z_]
evidence: src/frontend/preprocessor/README.md:271-275, src/frontend/preprocessor/conditionals.rs:795-804
```

## `eval_const_expr` selects the correct ternary branch for known conditions
- Tier: 4
- Rationale: The parser implements `?:` in `parse_ternary`, and the README lists ternary expressions as supported. For a condition whose truth value is already known from a sourced literal, the result should match the corresponding branch. Stronger considered: state machine (rejected — no lifecycle/state), differential (rejected — no alternative evaluator). Evidence: `src/frontend/preprocessor/README.md:277-294`, `src/frontend/preprocessor/conditionals.rs:481-490`.
- Test file: `src/frontend/preprocessor/conditionals.rs`
- Status: passing
- Counterexample: (none)

```property
function: frontend::preprocessor::eval_const_expr
oracle: reference
predicate:
  quantifier: forall
  vars: [cond_case, then_expr, else_expr]
  body: '{ let (cond, cond_truth) = cond_case; eval_const_expr(&format!("({} ? {} : {})", cond, then_expr, else_expr)) == (if cond_truth { eval_const_expr(&then_expr) } else { eval_const_expr(&else_expr) }) }'
generators:
  cond_case:
    gen: oneof
    options:
      - { gen: const, value: ["0", false] }
      - { gen: const, value: ["1", true] }
      - { gen: const, value: ["42", true] }
      - { gen: const, value: ["0x10", true] }
      - { gen: const, value: ["07", true] }
      - { gen: const, value: ["true", true] }
      - { gen: const, value: ["false", false] }
      - { gen: const, value: ["'\\0'", false] }
      - { gen: const, value: ["'\\n'", true] }
  then_expr:
    gen: recursive
    depth: 3
    maxSize: 96
    branchFactor: 16
    base:
      gen: oneof
      options:
        - { gen: const, value: "0" }
        - { gen: const, value: "1" }
        - { gen: const, value: "42" }
        - { gen: const, value: "0U" }
        - { gen: const, value: "0x10" }
        - { gen: const, value: "0x8000000000000000" }
        - { gen: const, value: "07" }
        - { gen: const, value: "3ULL" }
        - { gen: const, value: "'a'" }
        - { gen: const, value: "'\\n'" }
        - { gen: const, value: "'\\0'" }
        - { gen: const, value: "true" }
        - { gen: const, value: "false" }
  else_expr:
    gen: recursive
    depth: 3
    maxSize: 96
    branchFactor: 16
    base:
      gen: oneof
      options:
        - { gen: const, value: "0" }
        - { gen: const, value: "1" }
        - { gen: const, value: "42" }
        - { gen: const, value: "0U" }
        - { gen: const, value: "0x10" }
        - { gen: const, value: "0x8000000000000000" }
        - { gen: const, value: "07" }
        - { gen: const, value: "3ULL" }
        - { gen: const, value: "'a'" }
        - { gen: const, value: "'\\n'" }
        - { gen: const, value: "'\\0'" }
        - { gen: const, value: "true" }
        - { gen: const, value: "false" }
evidence: src/frontend/preprocessor/README.md:283-294, src/frontend/preprocessor/conditionals.rs:481-490, src/frontend/preprocessor/conditionals.rs:795-804
```
