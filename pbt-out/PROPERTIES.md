# Property Ledger: `src/frontend/lexer/scan.rs`

## `Lexer::tokenize` preserves token kinds across skipped regions
- Tier: 3
- Rationale: The lexer contract explicitly says it skips ASCII whitespace, line markers, line comments, and block comments before dispatching on the next token. A metamorphic property that compares token kind sequences before and after inserting only skipped regions exercises the real skip logic more strongly than a shallow unit example. Stronger considered: state machine (rejected — no explicit lifecycle/state transitions), differential (rejected — no second implementation). Evidence: `src/frontend/lexer/README.md:144`, `src/frontend/lexer/README.md:156`, `src/frontend/lexer/README.md:161`, `src/frontend/lexer/README.md:165`, `src/frontend/lexer/scan.rs:76`, `src/frontend/lexer/scan.rs:127`.
- Test file: `src/frontend/lexer/scan.rs`
- Status: approved
- Counterexample: (none)

```property
function: frontend::lexer::Lexer::tokenize
oracle: algebraic.metamorphic
predicate:
  quantifier: forall
  vars: [fragments, separators]
  relation:
    op: eq
    lhs: lex_kinds(join_with_separators(fragments, separators))
    rhs: lex_kinds(fragments.join(" "))
generators:
  fragments: { gen: list, elem: { gen: oneof, options: [{ gen: const, value: "foo" }, { gen: const, value: "bar" }, { gen: const, value: "_x" }, { gen: const, value: "$d" }, { gen: const, value: "int" }, { gen: const, value: "return" }, { gen: const, value: "42" }, { gen: const, value: "0x1f" }, { gen: const, value: "3.14" }, { gen: const, value: "+" }, { gen: const, value: "-" }, { gen: const, value: "*" }, { gen: const, value: "=" }, { gen: const, value: "==" }, { gen: const, value: "&&" }, { gen: const, value: "..." }, { gen: const, value: "->" }, { gen: const, value: "(" }, { gen: const, value: ")" }, { gen: const, value: ";" }] }, minLen: 1, maxLen: 12 }
  separators: { gen: list, elem: { gen: oneof, options: [{ gen: const, value: " " }, { gen: const, value: "\t" }, { gen: const, value: "\n" }, { gen: const, value: "/*c*/" }, { gen: const, value: "//c\n" }, { gen: const, value: "\n# 1 \"marker.c\"\n" }] }, minLen: 0, maxLen: 11 }
evidence: src/frontend/lexer/README.md:144-165, src/frontend/lexer/scan.rs:76, src/frontend/lexer/scan.rs:127
```

## `Lexer::set_gnu_extensions` controls bare GNU keyword recognition
- Tier: 4
- Rationale: The lexer README explicitly documents that bare `typeof` and `asm` are keywords only when `gnu_extensions` is enabled, while `__typeof__` and `__asm__` remain keywords in strict mode. This is a direct spec table, so a reference oracle is appropriate. Stronger considered: differential (rejected — no second implementation), algebraic (rejected — this is a finite closed mapping, not an inverse/idempotence/metamorphic law). Evidence: `src/frontend/lexer/README.md:332`, `src/frontend/lexer/README.md:334`, `src/frontend/lexer/README.md:336`.
- Test file: `src/frontend/lexer/scan.rs`
- Status: approved
- Counterexample: (none)

```property
function: frontend::lexer::Lexer::set_gnu_extensions
oracle: reference
predicate:
  quantifier: forall
  vars: [word, gnu_extensions]
  relation:
    op: eq
    lhs: lex_single_token_kind(word, gnu_extensions)
    rhs: expected_keyword_kind(word, gnu_extensions)
generators:
  word: { gen: oneof, options: [{ gen: const, value: "typeof" }, { gen: const, value: "asm" }, { gen: const, value: "__typeof__" }, { gen: const, value: "__asm__" }] }
  gnu_extensions: { gen: bool }
evidence: src/frontend/lexer/README.md:332-336
```

## `Lexer::tokenize` emits one trailing EOF with monotonic spans
- Tier: 3
- Rationale: `tokenize()` is documented as repeatedly calling `next_token()` until `Eof`, and every token carries a byte-offset `Span`. This invariant checks the real lexer over generated valid token/comment fragments and catches missing EOF, duplicate EOF, out-of-bounds spans, and non-monotonic source locations. Stronger considered: state machine (rejected — no explicit public state lifecycle), differential (rejected — no independent lexer implementation), round-trip/idempotence (rejected — lexer has no serializer/in-place normalizer). Evidence: `src/frontend/lexer/README.md:132`, `src/frontend/lexer/README.md:135`, `src/frontend/lexer/README.md:287`.
- Test file: `src/frontend/lexer/scan.rs`
- Status: approved
- Counterexample: (none)

```property
function: frontend::lexer::Lexer::tokenize
oracle: algebraic.invariant
predicate:
  quantifier: forall
  vars: [source_fragments]
  relation:
    op: holds
    expr: has_single_trailing_eof_and_monotonic_in_bounds_spans(Lexer::new(source_fragments.concat(), 0).tokenize())
generators:
  source_fragments: { gen: list, elem: { gen: oneof, options: [{ gen: const, value: "foo" }, { gen: const, value: "$bar" }, { gen: const, value: "int" }, { gen: const, value: "return" }, { gen: const, value: "42" }, { gen: const, value: "0x2a" }, { gen: const, value: "3.0" }, { gen: const, value: "\"s\"" }, { gen: const, value: "'c'" }, { gen: const, value: "+" }, { gen: const, value: "-" }, { gen: const, value: "*" }, { gen: const, value: "/" }, { gen: const, value: "%" }, { gen: const, value: "=" }, { gen: const, value: "==" }, { gen: const, value: "&&" }, { gen: const, value: "||" }, { gen: const, value: "..." }, { gen: const, value: "->" }, { gen: const, value: "(" }, { gen: const, value: ")" }, { gen: const, value: "{" }, { gen: const, value: "}" }, { gen: const, value: ";" }, { gen: const, value: " " }, { gen: const, value: "\t" }, { gen: const, value: "\n" }, { gen: const, value: "/*c*/" }, { gen: const, value: "//c\n" }, { gen: const, value: "\n# 7 \"marker.c\"\n" }] }, minLen: 0, maxLen: 64 }
evidence: src/frontend/lexer/README.md:132-135,287
```
