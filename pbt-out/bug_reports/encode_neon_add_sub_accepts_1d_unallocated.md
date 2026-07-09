# Bug: `encode_neon_add_sub` accepts `.1d` arrangement (UNALLOCATED)

## Summary
The `.1d` arrangement (size=0b11, Q=0) is not in the ARMv8-A ARM assembler-symbol set for vector ADD/SUB. The encoder silently accepts it and emits an instruction word that decodes as UNALLOCATED on hardware.

## Witness
```
cargo test -- --ignored add_sub_accepts_nonstandard_1d_arrangement
```

## Root cause
The arrangement-to-size mapping does not filter out `.1d`; any arrangement that maps to a valid size/Q combination is accepted without cross-checking the ARM ARM's valid-arrangement table.

## Severity
MEDIUM — emits UNALLOCATED encoding for an invalid assembly syntax that should be rejected.
