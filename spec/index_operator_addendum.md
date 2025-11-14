# Index Operator Addendum - Replacing count/name with [startindex-endindex] Syntax

## Overview

This addendum specifies the replacement of the `count=<number>` and `name=<format>` attribute pattern for defining indexed register fields with a more concise indexing operator syntax using brackets `[startindex-endindex]` directly on the `field_tag`.

## Current State Analysis

### Existing Syntax Pattern

The current specification (Section 9.1.1) defines indexed register fields using two separate attributes:

```
:<space_tag> <field_tag> [count=<number>] [name=<format>] [other_attributes...]
```

**Example from current codebase:**
```isa
:reg SPR offset=0x1000 size=64 count=1024 name=spr%d
```

This creates registers: `spr0`, `spr1`, `spr2`, ..., `spr1023`

### Identified Issues

1. **Verbosity**: Requires two separate attributes (`count` and `name`) to achieve indexing
2. **Format String Dependency**: Relies on printf-style format strings (`%d`) which limits naming flexibility
3. **Index Range Clarity**: Count doesn't clearly communicate the actual index range (always starts from 0)
4. **Inconsistency**: Another syntax already exists in the codebase (`gpr[0-31]`) suggesting users prefer the bracket notation

## Proposed Changes

### New Indexing Operator Syntax

Replace the `count=` and `name=` pattern with bracket notation directly on the field tag:

```
:<space_tag> <field_tag>[startindex-endindex] [other_attributes...]
```

**Examples:**
```isa
# Instead of: :reg SPR offset=0x1000 size=64 count=1024 name=spr%d
:reg SPR[0-1023] offset=0x1000 size=64

# Instead of: :reg GPR count=32 name=r%d  
:reg GPR[0-31]

# Single register (no indexing needed)
:reg PC
```

### Grammar Specification

#### Syntax Definition
```bnf
indexed_field_tag := field_tag '[' start_index '-' end_index ']'
field_tag         := single_word
start_index       := numeric_literal  
end_index         := numeric_literal
```

#### Validation Rules
1. `start_index` must be ≥ 0
2. `end_index` must be ≥ `start_index`  
3. `end_index - start_index + 1` must be ≤ 65535 (to fit in 16-bit unsigned integer)
4. Both indices must be valid numeric literals (decimal, hex, binary, octal)
5. The bracket notation `[startindex-endindex]` is mutually exclusive with `count=` and `name=` attributes

#### Field Name Generation
- Generated field names follow the pattern: `<field_tag><index>`
- Examples:
  - `SPR[0-1023]` � `SPR0`, `SPR1`, `SPR2`, ..., `SPR1023`
  - `GPR[0-31]` � `GPR0`, `GPR1`, `GPR2`, ..., `GPR31`
  - `r[10-15]` � `r10`, `r11`, `r12`, `r13`, `r14`, `r15`

## Specification Changes Required

### Section 9.1.1 Field Definition Syntax Forms

**REMOVE** from "New Field Options":
- `count=<numeric_literal>`: Number of registers in the file (for register arrays)
- `name=<format>`: Printf-style format for naming fields

**ADD** to "New Field Definition" syntax:
```
:<space_tag> <field_tag>[<start_index>-<end_index>] [offset=<numeric_literal>] [size=<bits>] [reset=<value>] [descr="<description>"] [subfields={list of subfield definitions}]
```

**ADD** new validation rule in Section 9.1.3:
- **Index Range Validation**: When using bracket notation, `start_index` ≤ `end_index`, both must be ≥ 0, and the total count (`end_index - start_index + 1`) must be ≤ 65535
- **Mutually Exclusive Attributes**: Bracket notation cannot be used with `count=` or `name=` attributes

### Section 5.1.4 Single Word Definition

**UPDATE** to clarify that single words can include bracket notation for field tags:
```
Single Word: Can contain upper and lower case letters, numbers, hyphens, underscores, periods. When used as a field_tag, may include indexing notation [startindex-endindex] for defining register arrays.
```

### Section 9.1.4 Field Examples

**REPLACE** current example:
```isa
# Old syntax
:reg GPR offset=0x100 size=64 count=32 name=r%d reset=0

# New syntax  
:reg GPR[0-31] offset=0x100 size=64 reset=0
```

**ADD** additional examples:
```isa
:space reg addr=32 word=64 type=register

# Index range starting from non-zero
:reg SPR[256-511] offset=0x1000 size=32

# Hex indices for special register ranges
:reg MSR[0x0-0xF] offset=0x2000 size=64

# Binary indices (though less practical)
:reg FLAGS[0b0-0b111] size=8
```

## Implementation Locations

### Tokenizer Changes
**File**: `server/src/tokenizer.ts` (assumed location)
- **ADD**: Token type for bracket notation `[startindex-endindex]`
- **UPDATE**: Field tag token recognition to include optional bracket suffix
- **ADD**: Validation for numeric literals within brackets

### Parser Changes  
**File**: `server/src/parser.ts` (assumed location)
- **ADD**: Parse bracket notation in field definitions
- **REMOVE**: Parse `count=` and `name=` attributes in field context
- **ADD**: Generate field name list from bracket notation
- **ADD**: Validation logic for index ranges

### Semantic Analyzer Changes
**File**: `server/src/semantic-analyzer.ts` (assumed location)
- **ADD**: Validation that bracket notation indices are within valid ranges
- **ADD**: Error reporting for invalid index ranges
- **REMOVE**: Validation logic for `count=` and `name=` combinations
- **UPDATE**: Field name resolution to handle generated indexed names

### Language Server Features
**File**: `server/src/completion-provider.ts` (assumed location)
- **ADD**: Auto-completion for bracket notation syntax
- **UPDATE**: Field name suggestions to include generated indexed names

**File**: `server/src/hover-provider.ts` (assumed location)  
- **ADD**: Hover information showing generated field names for indexed definitions

## Testing Requirements

### Unit Tests

#### Tokenizer Tests
**File**: `server/test/tokenizer.spec.ts`
```typescript
describe('Index Operator Tokenization', () => {
  test('should tokenize field_tag with bracket notation', () => {
    // Test: "GPR[0-31]" � [IDENTIFIER("GPR"), LBRACKET, NUMBER(0), DASH, NUMBER(31), RBRACKET]
  });
  
  test('should handle hex indices in brackets', () => {
    // Test: "MSR[0x0-0xF]" � tokens with hex number recognition
  });
  
  test('should reject malformed bracket notation', () => {
    // Test: "GPR[0-]", "GPR[-31]", "GPR[0--31]" should produce errors
  });
});
```

#### Parser Tests  
**File**: `server/test/parser.spec.ts`
```typescript
describe('Index Operator Parsing', () => {
  test('should parse indexed field definition', () => {
    const input = ':reg GPR[0-31] size=64';
    // Should produce AST with indexed field node
  });
  
  test('should generate correct field names', () => {
    const input = ':reg r[10-12] size=32';
    // Should generate: r10, r11, r12
  });
  
  test('should reject count/name with bracket notation', () => {
    const input = ':reg GPR[0-31] count=32 name=r%d';
    // Should produce semantic error
  });
});
```

#### Semantic Analysis Tests
**File**: `server/test/semantic-analyzer.spec.ts`
```typescript
describe('Index Operator Validation', () => {
  test('should validate index ranges', () => {
    // Valid: [0-31], [10-15], [0x0-0xF]
    // Invalid: [31-0], [-1-10], [0-65537]
  });
  
  test('should resolve indexed field references', () => {
    // Test alias references to generated field names
    const input = ':reg GPR[0-31]\n:reg SP alias=GPR1';
    // Should validate GPR1 exists
  });
});
```

### Integration Tests

#### Example File Validation
**File**: `server/test/integration/examples.spec.ts`
```typescript
describe('Example Files with Index Operator', () => {
  test('should migrate alias.isa to new syntax', () => {
    // Convert: :reg SPR offset=0x1000 size=64 count=1024 name=spr%d
    // To: :reg SPR[0-1023] offset=0x1000 size=64
    // Validate all existing aliases still work
  });
  
  test('should handle existing bracket notation in valid-file.isa', () => {
    // Ensure gpr[0-31] continues to work correctly
  });
});
```

### Migration Tests

#### Backward Compatibility Tests
**File**: `server/test/migration/count-name-migration.spec.ts`
```typescript
describe('Count/Name Migration', () => {
  test('should provide helpful error for old syntax', () => {
    const input = ':reg GPR count=32 name=r%d';
    // Should suggest migration to :reg GPR[0-31]
  });
  
  test('should handle mixed old/new syntax files', () => {
    // Test behavior when both syntaxes appear in same file
  });
});
```

## Example File Updates Required

### Update alias.isa
**Current (Line 9)**:
```isa
:reg SPR offset=0x1000 size=64 count=1024 name=spr%d
```

**New**:
```isa
:reg SPR[0-1023] offset=0x1000 size=64
```

**Test Cases to Verify**:
-  `spr9` reference in `:reg CTR alias=spr9` (line 18)
-  `spr22` reference in `:reg DEC alias=spr22::lsb` (line 23)  
- L `spr1024` reference should still error (line 28) - now "SPR1024 not defined" instead of "out of range"

### Update Other Examples
Review and update any other example files that may use the `count=` and `name=` pattern to ensure consistency with the new syntax.

## Language Server Protocol Considerations

### Diagnostics
- **Error Codes**: Add new error codes for invalid bracket notation
- **Error Messages**: Clear messages suggesting correct syntax
- **Quick Fixes**: Offer automatic migration from old syntax to new syntax

### Completions
- **Bracket Notation**: Auto-complete `[0-` when typing after field tag
- **Index Suggestions**: Suggest common index ranges like `[0-31]`, `[0-15]`, etc.

### Hover Information
- **Generated Names**: Show list of generated field names when hovering over indexed definition
- **Range Info**: Display index range and total count

## Timeline and Prioritization

### Phase 1: Core Implementation
1. **Tokenizer updates** for bracket notation recognition
2. **Parser updates** for indexed field syntax  
3. **Basic validation** for index ranges

### Phase 2: Semantic Features
1. **Field name generation** and resolution
2. **Alias validation** for generated names
3. **Error reporting** improvements

### Phase 3: Language Server Features  
1. **Auto-completion** for bracket notation
2. **Hover information** for indexed fields
3. **Quick fixes** for migration

### Phase 4: Migration Support
1. **Deprecation warnings** for old syntax
2. **Migration tools** for existing files
3. **Documentation updates**

## Summary

This change simplifies the ISA language syntax by:
- **Reducing verbosity**: One bracket notation instead of two attributes
- **Improving clarity**: Explicit index ranges instead of count + format string
- **Enhancing consistency**: Aligns with existing `gpr[0-31]` syntax found in codebase
- **Maintaining functionality**: All current use cases remain supported

The migration requires updates to tokenizer, parser, semantic analyzer, and comprehensive testing to ensure existing functionality is preserved while providing a more intuitive syntax for register array definitions.
