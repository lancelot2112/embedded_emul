# Alias Option Tag Rename Addendum

## Problem Statement

The current `alias=` option tag in field definitions is misleading about its actual function. The term "alias" suggests a simple name redirect, but the feature actually creates a new type that shares the same memory characteristics (offset, size, bit layout) as the referenced field while having completely independent typing information (subfields, operational properties).

This is conceptually different from a true alias, which would typically inherit all properties of the original. Instead, this mechanism creates a memory-sharing type relationship where:
- **Memory characteristics are inherited**: offset, size, bit positions
- **Type information is independent**: subfields, operational semantics, descriptions

## Current Syntax

```plaintext
:reg CTR alias=spr9
:reg DEC alias=spr22::lsb
```

## Proposed Change

Replace the `alias=` option tag with `redirect=` to better reflect its purpose as a memory redirection mechanism that creates a new type sharing the same memory space.

### New Syntax

```plaintext
:reg CTR redirect=spr9
:reg DEC redirect=spr22::lsb
```

## Rationale

The term "redirect" more accurately describes the behavior:
1. **Memory Redirection**: The new field redirects to the same memory location as the referenced field
2. **Type Independence**: The redirected field can have its own subfields, descriptions, and operational semantics
3. **Clear Intent**: Indicates that this creates a new view of existing memory rather than a simple name alias

## Benefits

1. **Semantic Clarity**: The name better reflects what the feature actually does
2. **Developer Understanding**: Reduces confusion about whether type information is inherited
3. **Future Consistency**: Aligns with the concept that different types can share memory without sharing type definitions
4. **Documentation Accuracy**: Makes specification language more precise

## Implementation Impact

### Language Specification Changes
- Update Section 9.1.1 "Alias Definition" to "Redirect Definition"
- Replace all instances of `alias=` with `redirect=` in syntax specifications
- Update validation rules to reference `redirect` instead of `alias`

### Example File Updates
- Update `examples/alias.isa` to use `redirect=` syntax
- Update comments to reflect the memory redirection concept

### Tooling Changes
- Language server must recognize `redirect=` as valid syntax
- Syntax highlighting should apply same rules to `redirect=` as current `alias=`
- Error messages should reference "redirect" terminology

## Migration Strategy

### Backward Compatibility Period
1. Support both `alias=` and `redirect=` during transition period
2. Emit deprecation warnings for `alias=` usage
3. Update all example files to use `redirect=`

### Validation Rules
The same validation rules apply:
- `redirect` cannot be used with `offset`, `size`, `count`, `name`, or `reset`
- Referenced field must exist and be previously defined
- Context operator syntax (`::`) is supported for subfield references

## Updated Examples

### Basic Field Redirection
```plaintext
:space reg addr=32 word=64 type=register

:reg SPR offset=0x1000 size=64 count=1024 name=spr%d
:reg CTR redirect=spr9  # Creates new type sharing spr9's memory
```

### Subfield Redirection
```plaintext
:reg DEC redirect=spr22::lsb  # Creates new type sharing spr22's lsb subfield memory
```

### With Independent Subfields
```plaintext
:reg CTR redirect=spr9 subfields={
    counter_bits @(0-31) op=imm descr="Counter value"
    reserved @(32-63) descr="Reserved bits"
}
```

This demonstrates how the redirected field can have its own type information (subfields) while sharing the memory location of spr9.