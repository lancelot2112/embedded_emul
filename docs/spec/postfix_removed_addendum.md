# Postfix Operation Removal Addendum

## Summary
This addendum removes the postfix operation syntax (`?<postfix>`) from the ISA language specification. This change simplifies the subfield definition syntax and removes the complex postfix handling logic from instruction disassembly.

## Changes to Section 9.1.2 Subfields

### Original Subfield Syntax (REMOVED)
```
<subfield_tag>[?<postfix>] @(<bit_spec>)[|<bit_spec>...] [op=<type>[.<subtype>][|<type>...]] [descr="<description>"]
```

### New Subfield Syntax
```
<subfield_tag> @(<bit_spec>)[|<bit_spec>...] [op=<type>[.<subtype>][|<type>...]] [descr="<description>"]
```

## Removed Components

### Postfix Syntax
- **REMOVED** `?<postfix>`: Optional single-character postfix (e.g., `?a`, `?l`) is no longer supported
- **REMOVED** Postfix appending to field_name or instruction_name during disassembly
- **REMOVED** Single-bit requirement for postfix fields

## Impact on Existing Examples

### Before (with postfix)
```plaintext
:insn subfields={
    AA?a @(30) op=func descr="Absolute Address flag, bit 30"
    LK?l @(31) op=func descr="Link flag, bit 31"
    OE?o @(21) op=func descr="Overflow Enable flag, bit 21"
    Rc?. @(31) op=func descr="Record flag, bit 31"
}
```

### After (without postfix)
```plaintext
:insn subfields={
    AA @(30) op=func descr="Absolute Address flag, bit 30"
    LK @(31) op=func descr="Link flag, bit 31"
    OE @(21) op=func descr="Overflow Enable flag, bit 21"
    Rc @(31) op=func descr="Record flag, bit 31"
}
```

## Migration Guide

1. **Remove postfix characters**: Remove the `?<character>` portion from all subfield definitions
2. **Update field names**: If the postfix character was meaningful, incorporate it into the base `<subfield_tag>` name
3. **Update disassembly logic**: Any code that relied on postfix-based instruction name modification must be updated to handle field-based logic instead

## Rationale

- **Simplification**: Removes complex postfix handling logic from parsers and disassemblers
- **Clarity**: Subfield names are now explicit rather than having dynamic postfix behavior
- **Consistency**: Eliminates special-case handling for single-bit postfix fields
- **Maintainability**: Reduces parser complexity and potential edge cases

## Effective Date
This change takes effect immediately upon adoption of this addendum. All new ISA files should follow the updated syntax without postfix operations.