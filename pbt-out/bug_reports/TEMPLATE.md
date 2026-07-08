# Bug Report Template

Use this consistent format for all bug reports:

```markdown
# Bug Report: `<function_name>` `<brief_description>`

**Target:** `<file_path>` → `<function_name>`
**Severity:** `<severity_level>`

## Summary

<1-3 paragraph description of what the bug is and why it's a problem>

## Root Cause

<Technical explanation of the bug with code snippet showing the problematic code>

## Reproduction

<Example input that demonstrates the bug, with expected vs actual behavior>

## Impact

<Why this matters - silent mis-compilation, security implications, etc.>

## Suggested Fix

<Code or description of how to fix the bug>

## Regression Property

Failing property: `<property_name>`

```rust
<Minimal test case that fails>
```

**GitHub Issue:** <link if created>
```

## Guidelines

- **Severity levels:** High, Medium, Low
- **Title format:** `# Bug Report: <function> <brief description>`
- **Section order:** Summary → Root Cause → Reproduction → Impact → Suggested Fix → Regression Property
- **Root Cause:** Include the problematic code snippet
- **Reproduction:** Show actual input and output demonstrating the bug
- **Impact:** Explain consequences of the bug
- **Suggested Fix:** Provide concrete fix (code or clear description)
- **Regression Property:** Always include if property exists, with minimal failing test
- **GitHub Issue:** Add link if issue has been created

## Common Inconsistencies to Fix

1. **Title variations:**
   - ✅ `# Bug Report: encode_add_sub silently truncates extended-register shift (imm3)`
   - ❌ `# Bug — encode_add_sub silently truncates...`
   - ❌ `# BUG: encode_add_sub...`
   - ❌ `# Bug Report — encode_add_sub...`

2. **Header fields:**
   - ✅ Use `**Target:**` and `**Severity:**`
   - ❌ Don't mix `**Location:**`, `**File:**`, `**Status:**`, etc.

3. **Section headings:**
   - ✅ Use `## Root Cause`, `## Reproduction`, `## Impact`, `## Suggested Fix`
   - ❌ Don't use `## Where`, `## Problem`, `## Witness`, etc.

4. **Regression property:**
   - ✅ Use `## Regression Property` section with `Failing property:`
   - ❌ Don't use `## Regression property`, `## Failing property`, etc.