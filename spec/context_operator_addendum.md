# ISA Language Specification Addendum: Context Operator

## Overview

This addendum introduces a syntax change to improve tokenization clarity and eliminate operator overloading issues with the period (`.`) character in the ISA language specification.

## Problem Statement

The current ISA language specification uses the period (`.`) character for multiple purposes:
1. **Field.subfield references**: `spr22.lsb` 
2. **Valid identifier characters**: Periods are allowed within field and subfield names
3. **Space indirection**: `$space->field` (arrow operator uses hyphen and greater-than)

This overloading creates tokenization ambiguity and complicates parsing, especially when determining whether a period separates tokens or is part of an identifier.

## Solution: Context Operator (`::`)

We propose introducing the semicolon (`::`) as the universal **context operator** for all hierarchical references in the ISA language.

### Rationale

1. **Unambiguous tokenization**: Semicolon is not used elsewhere in the language
2. **Consistent hierarchy**: Single operator for all context switching
3. **Clear semantics**: "Evaluate what follows in the context of what precedes"
4. **Future extensibility**: Supports deeper hierarchies if needed

## Syntax Changes

### 1. Field-Subfield References

**Current syntax:**
```isa
:reg DEC alias=spr22.lsb
```

**New syntax:**
```isa
:reg DEC alias=spr22::lsb
```

### 2. Space Indirection

**Current syntax:**
```isa
mask={$reg->spr22.lsb=1}
```

**New syntax:**
```isa
mask={$reg::spr22::lsb=1}
```

### 3. Extended Context Chains

The context operator supports chaining for complex references:

```isa
# Space to field to subfield
$reg::spr22::lsb

# Future: Could support deeper hierarchies
$core::reg::spr22::lsb
```

## Semantic Interpretation

The context operator (`;`) creates a **left-to-right evaluation chain**:

1. `field::subfield` → Find `subfield` within the scope of `field`
2. `$space::field` → Find `field` within the scope of `space`
3. `$space::field::subfield` → Find `subfield` within `field` within `space`

## Updated Grammar

### Context Reference
```
context_reference := base_context ('::' context_element)*
base_context      := space_reference | identifier
context_element   := identifier
space_reference   := '$' identifier
```

### Examples in Grammar Context

```
# Field aliases with subfield references
field_alias := 'alias=' context_reference

# Instruction mask assignments  
mask_assignment := context_reference '=' numeric_literal

# Operand references
operand_list := '(' (context_reference (',' context_reference)*)? ')'
```

## Benefits

### 1. Improved Tokenization
- Periods can remain valid identifier characters without ambiguity
- Tokenizer can clearly distinguish operators from identifier content
- Simplifies lexical analysis rules

### 2. Consistent Syntax
- Single operator for all hierarchical references
- Unified mental model for context switching
- Consistent with programming language conventions (namespace operators)

### 3. Enhanced Readability
- Clear visual separation of context levels
- Easier to parse visually: `$reg::spr22::lsb` vs `$reg->spr22.lsb`
- Consistent operator precedence and associativity

### 4. Future Compatibility
- Supports potential language extensions (deeper hierarchies)
- Room for additional operators without conflicts
- Easier to extend parser for new features

## Migration Impact

### Low Impact Changes
- **Parser updates**: Replace `.` and `->` parsing with `::` operator
- **Tokenizer updates**: Treat `::` as operator, allow `.` in identifiers
- **Validation updates**: Update reference resolution logic

### Example Files Updates
All example files would need syntax updates:

**alias.isa changes:**
```diff
- :reg DEC alias=spr22.lsb
+ :reg DEC alias=spr22::lsb

- :reg SUBFIELD_NOT_DEFINED alias=spr22.NDF  
+ :reg SUBFIELD_NOT_DEFINED alias=spr22::NDF

- mask={$reg->spr22.lsb=1}
+ mask={$reg::spr22::lsb=1}
```

## Implementation Notes

### Tokenizer Changes
1. Remove `.` from operator tokens
2. Add `::` as CONTEXT_OPERATOR token type
3. Allow `.` in IDENTIFIER token patterns

### Parser Changes  
1. Replace field.subfield parsing with field::subfield
2. Replace space->field parsing with space::field
3. Update context reference resolution logic

### Validation Changes
1. Update alias validation to use `::` separator
2. Update space indirection validation 
3. Update error messages to reflect new syntax

## Backward Compatibility

This change is **breaking** and requires:
1. Update all existing ISA files to new syntax
2. Update language server parser and tokenizer
3. Update VS Code extension syntax highlighting
4. Update documentation and examples

## Timeline

1. **Phase 1**: Update specification and get approval
2. **Phase 2**: Implement parser and tokenizer changes
3. **Phase 3**: Update all example files and test cases
4. **Phase 4**: Update syntax highlighting and LSP features
5. **Phase 5**: Documentation updates and testing

## Conclusion

The context operator (`::`) provides a cleaner, more consistent syntax for hierarchical references in the ISA language while eliminating tokenization ambiguities. This change improves both human readability and machine parsing reliability.