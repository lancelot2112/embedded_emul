# .isa File Format Specification (Revised)

## 1. Introduction
This specification defines a series of files used to describe a full system of interacting cores, memories, and bus accesses.  The format is text-based and declarative and aims to be human-readable and parsable for use in emulators, disassemblers, and other CPU modeling tools.

## 2. File Types
The `.isa` file format is designed to describe the Instruction Set Architecture (ISA) of a processor. This includes definitions for memory spaces, register files, register fields, instruction fields, and instruction opcodes/formats.

`.isaext` files allow extensions to or additions to other `.isa` or `.isaext`.  This means that the file may use symbols that are defined in other files.  `.isaext` will need special consideration as far as linting and error reporting since it doesn't explicitly include the other files itself.

`.core` files include a base `.isa` and any number of `.isaext` plus can add extensions to the isa that are core specific.  These files should also be validated per the isa standard but include the context of the included files.  These files can define core specific logic and fields using the isa standard. Linting this file is where `.isaext` should be validated against the context of the other files included.

`.sys` files contain a list of `.core` files also using a sys file specific `:attach` command.  Other isa file contructs are also valid however each `.core` file creates its own context.  These files can define system specific logic and fields using the isa standard.

## 3. Validation and Error Handling
### 3.1 General Validation Rules

- All numeric literals must conform to the specified formats
- Field names and space tags must be unique within their scope
- Bit indices must be within the valid range for their container
- References to fields, spaces, and subfields must be to previously defined entities (special consideration for `.isaext` files)
- Form type references must resolve to valid forms within the same logic space
- Cross-space form typing is not permitted
- Mask constraints must reference valid form fields
- Logic spaces cannot contain `offset` attributes

### 3.2 Error Types

- **Syntax Errors**: Malformed directives, invalid numeric literals, missing required attributes, unclosed contexts and subcontexts
- **Semantic Errors**: References to undefined entities, bit indices out of range, conflicting definitions, invalid form types, cross-space typing attempts
- **Warning Conditions**: Overlapping field ranges, unused definitions, overlapping fields in inherited forms

### 3.3 Error Reporting

The linter should provide clear error messages with:
- Line number and column position
- Description of the error
- Suggestion for correction when possible


## 4. Color Scheme and Highlighting

A color scheme settings file will be provided that allows configuring colors for various parts of the language. Default colors are specified throughout this document for specific parts of the syntax and can be customized.  Coloring should be context aware and use the tokenizer.

## 5. General Syntax

Linting and coloring should both utilize a common tokenization scheme and avoid regex pattern matching.

### 5.1 Simple Types

#### 5.1.1 **Numeric Literals**: A literal defining a number that must use one of these formats:
  - **Hexadecimal**: `0x` followed by valid hex digits (0-9, a-f, A-F)
  - **Binary**: `0b` followed by valid binary digits (0-1)
  - **Octal**: `0o` followed by valid octal digits (0-7)
  - **Decimal**: Plain decimal digits (0-9)

  Any detected numeric literal is highlighted (default: `tan with a hint of green`).

#### 5.1.2 **Comments**: Anything after a `#` character in any line is a comment and should be ignored for linting. Comments are highlighted (default: `green`).

#### 5.1.3 **Quoted Strings**: Values containing spaces or special characters should be enclosed in double quotes (e.g., `"User mode"`). Strings are highlighted (default: `orange`).

#### 5.1.4 **Single Word**: Can contain upper and lower case letters, numbers, hyphens, underscores, periods. When used as a field_tag, may include indexing notation [startindex-endindex] for defining register arrays.

#### 5.1.5 **Bit Field**: Start with the `@` symbol and includes anything enclosed in the parenthesis just after `@(<bit_field>)`. For details see "Bit Specification Details".

##### 5.1.5.1 Bit Specification Details (`@(...)`)

Bit specifications are used in field definitions and instruction definitions. They define how a field maps to bits within a container (register or instruction word). The `containerSize` (from `:<space_tag> <field_tag> size=` or `:<space_tag> <instruction_tag> size=`) is the total width for bit numbering.

##### 5.1.5.2 Bit Numbering

- **Convention**: Assumed to be MSB 0 (Most Significant Bit is bit 0). Bit `N` refers to the Nth bit from the MSB.
- **Interpretation**: For a container of `W` bits (e.g., a 32-bit instruction), bit 0 is the MSB and bit `W-1` is the LSB.

##### 5.1.5.3 Syntax Forms

**Single Bit**:
- `@(<bit_index>)`: A single bit.
- Example: `AA @(30)` refers to bit 30.

**Bit Range**:
- `@(<start_bit>-<end_bit>)`: A contiguous range of bits, inclusive. `start_bit` is typically the more significant bit index.
- Example: `rA @(11-15)` refers to bits 11 through 15.
- Length of this segment is `end_bit - start_bit + 1`.

##### 5.1.5.4 Concatenation

**Multiple Segments**:
- `@(<spec1>|<spec2>|...)`: Multiple bit segments are extracted and concatenated in order to form the field's value.
- Example: `DCRN @(16-20|11-15)` takes bits 16-20, then appends bits 11-15.

**Literal Padding**:
- `@(<spec>|0b<binary_digits>)`: A bit segment is concatenated with literal binary digits.
- Example: `BD @(16-29|0b00)` takes bits 16-29 and appends `00` as the two least significant bits.

**Sign Extension**:
- `@(?<0 or 1>|<spec1>|<spec2>)` implies either 0 extending or sign extending.
- Example: `BX @(?1|16-29|0b00)` where bits 16-29 are set to 0x1FFF (bit16 being 1) will result in a value of 0xFFFFFFFC where the bits are sign extended to the left (assuming bit16 is the sign bit) and bits 00 are padded to the LSB.

##### 5.1.5.5 Field Value Interpretation

When concatenating, segments are shifted and ORed together to form the final field value. A field `@(S-E)` (where S is the MSB index, E is the LSB index of the field part) extracts bits from S to E.

### 5.2 Basic Language Constructs

#### 5.2.1 Contexts
- **Context Windows**: Each line starting with a `:<directive>` begins a new context window. Every line after the directive (until a new directive is declared) is within the context window. Directive context windows will never be nested.  The lines between two `:` directives should be able to be folded.

- **Subcontext Windows**: Subcontext windows can be opened up by:
  - An individual option tag `<optiontag>={muli-line optiontag context window}` using braces `{}`
  - Bit fields starting with the `@` tag `@(single line bitfield context window)` using `()`
  - Function declarations/calls `<functiontag><any number of spaces including 0>(multi-line function argument context window)` using `()`

  Each subcontext shall use the default linting and coloring rules unless explicitly overridden by requirements later in this document. Subcontext boundary characters `()` and `{}` shall be highlighted with a different color for each nesting level.

#### 5.2.2 Directives
- **Directives**: Directives start with a colon (`:`) followed by a directive keyword
  - **Basic Directives**: `:param` defines a parameter, `:space` defines a logical space, `:bus` defines a connection between spaces. Previously listed basic directives including the colon are highlighted (default: `blue gray`). Invalid directives will remain default text color.
  - **Space Directives**: Every `:space <space_tag>` defines a new `space declaration` directive which can be accessed by `:<space_tag>` anytime after a space is defined to declare named/typed `fields` or `instructions` within the space using `:<space_tag> <field_tag>` or `:<space_tag> <instruction_tag>`.

  `fields` and `instructions` will have their own unique linting requirements. Each `<space_tag>` shall get its own color. Space directives including the colon, field tags, and function tags, are highlighted according to the assigned color of the parent `<space_tag>`. Use of invalid tags and space directives (not previously defined in the file) shall remain the default text color and indicate an error.
  - **Core File Directives**: `:include` includes `.isa` and `.isaext` files that the core uses
  - **System File Directives**: `:attach` attaches cores to the system by referencing `:core` files

#### 5.2.3 References
- **Scoped Reference**: Each space contains a number of `field` or `instruction` declarations that are valid for reference by tag inside that space.  To change the scope to that space one can use the `space directive` indicating we are going to declare a field or instruction in that space.  
- **Context Reference**: Hierarchical references use the context operator (`::`) for all field and space indirection:
  - **Field-Subfield References**: `field::subfield` (e.g., `spr22::lsb`)
  - **Space Indirection**: `$space::field` or `$space::field::subfield` (e.g., `$reg::spr22::lsb`)
  - **Context Operator Semantics**: The double colon (`::`) creates a left-to-right evaluation chain where each element is resolved within the context of the preceding element

#### 5.2.4 Context Reference Grammar

The context operator (`::`) provides a unified syntax for hierarchical references:

```
context_reference := base_context ('::' context_element)*
base_context      := space_reference | identifier  
context_element   := identifier
space_reference   := '$' identifier
```

**Examples**:
- `spr22::lsb` - subfield `lsb` within field `spr22`
- `$reg::spr22` - field `spr22` within space `reg`
- `$reg::spr22::lsb` - subfield `lsb` within field `spr22` within space `reg`

**Tokenization**: The semicolon (`::`) is treated as a distinct operator token, allowing periods (`.`) to be used freely within identifier names without ambiguity.

#### 5.2.5 Index Operator Grammar

The index operator (`[startindex-endindex]`) provides syntax for defining register arrays:

```
indexed_field_tag := field_tag '[' start_index '-' end_index ']'
field_tag         := single_word
start_index       := numeric_literal  
end_index         := numeric_literal
```

**Validation Rules**:
1. `start_index` must be ≥ 0
2. `end_index` must be ≥ `start_index`  
3. `end_index - start_index + 1` must be ≤ 65535 (to fit in 16-bit unsigned integer)
4. Both indices must be valid numeric literals (decimal, hex, binary, octal)
5. The bracket notation `[startindex-endindex]` is mutually exclusive with deprecated `count=` and `name=` attributes

**Field Name Generation**:
- Generated field names follow the pattern: `<field_tag><index>`
- Examples:
  - `SPR[0-1023]` → `SPR0`, `SPR1`, `SPR2`, ..., `SPR1023`
  - `GPR[0-31]` → `GPR0`, `GPR1`, `GPR2`, ..., `GPR31`
  - `r[10-15]` → `r10`, `r11`, `r12`, `r13`, `r14`, `r15`

**Operator Precedence and Scoping**:
- Index operators are parsed as part of the field_tag token during tokenization
- Index ranges are evaluated at parse time to generate the complete list of field names
- Generated field names are available for alias references and space indirection
- The bracket notation has higher precedence than any field attributes
 
## 6. Global Parameters (`:param`)

Defines global parameters for the ISA.

- **Syntax**: `:param <NAME>=<VALUE>`
- **Value Validation**:
  - `<VALUE>` must be either a **numeric literal** or a **single word**
- **Example**:
  ```plaintext
  :param ENDIAN=big
  :param REGISTER_SIZE=32
  :param CACHE_SIZE=0x8000
  :param FLAGS=0b1101
  :param BASE_ADDR=0o777
  ```
- **Default Parameters**:
  - `ENDIAN`: Specifies the default endianness (`big` or `little`)
  - `REGISTER_SIZE`: Specifies a default register size in bits (though individual registers or spaces can override this)

## 7. Logical Memory Spaces (`:space`)

Defines logical address spaces, such as RAM, register banks, or memory-mapped I/O.

- **Syntax**: `:space <space_tag> [addr=<bits>] [word=<bits>] [type=<SpaceType>] [align=<bytes>] [endian=<Endianness>]`
- **Attributes**:
  - `<space_tag>`: Unique name for the space (e.g., `ram`, `reg`). Each `<space_tag>` shall get its own assigned color.
  - `addr=<bits>`: **REQUIRED** - Size of addresses within this space in bits. Must be a valid numeric literal (1-128 bits recommended).
  - `word=<bits>`: **REQUIRED** - Natural word size for this space in bits. Must be a valid numeric literal (1-128 bits recommended).
  - `type=<SpaceType>`: **REQUIRED** - Type of the space. Valid values:
    - `rw`: General read/write memory space
    - `ro`: Read only memory space
    - `memio`: Memory-mapped I/O space
    - `register`: CPU register space
    - `logic`: Instruction forms and instruction definitions for binary analysis and encoding
  - `align=<bytes>`: **OPTIONAL (default=16)** - Default alignment for accesses in this space in bytes. Must be a valid numeric literal.
  - `endian=<Endianness>`: **OPTIONAL (default=big if ENDIAN is not defined)** - Endianness for this space (`big` or `little`), overrides global `:param ENDIAN`.
- **Numeric Literal Validation**: All numeric values (`addr`, `word`, `align`) must be a valid **numeric literal**
- **Example**:
  ```plaintext
  :space ram addr=64 word=32 type=rw align=16 endian=big
  :space reg addr=0x20 word=0b1000000 type=register
  :space mmio addr=0o100 word=32 type=memio
  :space powerpc_insn type=logic size=32 endian=big
  ```

### 7.1 Logic Spaces and Instruction Forms

Logic spaces (`type=logic`) provide a specialized space type for defining instruction forms and instruction definitions used in binary analysis, disassembly, and encoding. Unlike other space types, logic spaces are not memory-mapped and serve as templates for instruction encoding patterns.

#### 7.1.1 Logic Space Characteristics

- **Purpose**: Defines instruction forms and instruction definitions for binary analysis and encoding
- **No bus mapping**: Logic spaces are not added to buses and have no physical memory representation
- **Binary scanning**: Forms and instructions defined in logic spaces are used to scan and interpret binary instruction encodings
- **Invalid attributes**: The `offset` attribute is invalid in logic spaces since there is no concept of memory mapping
- **Unified space**: Both forms and instructions coexist in the same logic space
- **Required attributes**: `type=logic`, `size=<bits>` (instruction width), optional `endian=<endianness>`

#### 7.1.2 Form Definition (Typing)

Forms act as type templates that instructions can be typed with, providing reusable encoding layouts for similar instruction patterns.

**Syntax**: 
```
:<space_name> <form_name> subfields={
    <field_name> @(<bit_range>) op=<operation_type> [descr="<description>"]
    ...
}
```

**Form Characteristics**:
- **Type templates**: Forms act as types that instructions can be typed with
- **Bitfield definitions**: Use standard bitfield syntax for encoding layouts
- **Operation types**: Standard operation types (func, target, source, immediate, etc.)
- **Documentation**: Support description strings for form documentation
- **Same-space only**: Forms can only be referenced within the same logic space

**Example Form Definitions**:
```isa
:space powerpc_insn type=logic size=32 endian=big

:powerpc_insn X_Form subfields={
    OPCD @(0-5) op=func descr="Primary opcode"
    RT @(6-10) op=target|reg.GPR descr="Target register"  
    RA @(11-15) op=source|reg.GPR descr="Source register A"
    RB @(16-20) op=source|reg.GPR descr="Source register B"
    XO @(21-30) op=func descr="Extended opcode"
    Rc @(31) op=func descr="Record condition"
}

:powerpc_insn D_Form subfields={
    OPCD @(0-5) op=func descr="Primary opcode"
    RT @(6-10) op=target|reg.GPR descr="Target register"
    RA @(11-15) op=source|reg.GPR descr="Source register"
    D @(16-31) op=immediate descr="Displacement"
}
```

#### 7.1.3 Form Inheritance

Forms can inherit from other forms within the same logic space, allowing extension and refinement of base encoding patterns.

**Syntax**:
```
:<space_name>::<parent_form> <child_form> subfields={
    <additional_field> @(<bit_range>) op=<operation_type> [descr="<description>"]
    ...
}
```

**Inheritance Rules**:
1. **Preserve bit ranges**: Inherited fields maintain their original bit range declarations
2. **Allow overlaps**: Child form fields may overlap with inherited fields
3. **Overlap warnings**: Language tools should warn when new fields overlap with inherited fields
4. **No greedy changes**: Bit ranges of inherited fields cannot be modified
5. **Force new forms**: For bit range modifications, create a new form rather than inheriting

**Example Form Inheritance**:
```isa
# Base X_Form
:powerpc_insn X_Form subfields={
    OPCD @(0-5) op=func descr="Primary opcode"
    RT @(6-10) op=target|reg.GPR descr="Target register"  
    RA @(11-15) op=source|reg.GPR descr="Source register A"
    RB @(16-20) op=source|reg.GPR descr="Source register B"
    XO @(21-30) op=func descr="Extended opcode"
    Rc @(31) op=func descr="Record condition"
}

# XO_Form inherits from X_Form and adds OE field
:powerpc_insn::X_Form XO_Form subfields={
    OE @(21) op=func descr="Overflow enable"
    # WARNING: OE @(21) overlaps with inherited XO @(21-30)
    # Both fields coexist with their declared bit ranges
}
```

#### 7.1.4 Typed Instructions

Instructions can be typed using the context operator with forms, providing automatic operand inference and encoding templates.

**Syntax**:
```
:<space_name>::<form_name> <instruction_name> [operand_list] [mask={<mask_specification>}] [descr="<description>"] [semantics={<semantics_block>}]
```

**Characteristics**:
- **Type specification**: Instructions declare their encoding type using `::form_name`
- **Same-space typing**: Form references are local to the logic space (no cross-space typing)
- **Automatic operand inference**: Operand list inferred from form fields with non-func operation types
- **Explicit operand override**: Instructions can provide explicit operand lists when needed

**Example Typed Instructions**:
```isa
# X_Form typed instructions
:powerpc_insn::X_Form add mask={OPCD=31, XO=266, Rc=0} descr="Add"
:powerpc_insn::X_Form add. mask={OPCD=31, XO=266, Rc=1} descr="Add and record"
:powerpc_insn::X_Form sub mask={OPCD=31, XO=40, Rc=0} descr="Subtract"

# Override operand list when needed
:powerpc_insn::X_Form mr (RT, RA) mask={OPCD=31, XO=444, RB=0, Rc=0} descr="Move register"

# D_Form typed instructions
:powerpc_insn::D_Form lwz mask={OPCD=32} descr="Load word and zero"
:powerpc_insn::D_Form addi mask={OPCD=14} descr="Add immediate"
```

**Example Grouping Typed Instructions**:
```isa
# Another options creating a group of instructions having the same form for brevity
$powerpc_insn::D_Form={
    +lwz mask={OPCD=32} descr="Load word and zero"
    +addi mask={OPCD=14} descr="Add immediate"
} 

**Exddample Grouping a Logic Space**
```isa
#or specifying the same logic space
:powerpc_insn={
    ::D_Form={
       +lwz mask={OPCD=32} descr="Load word and zero"
       +addi mask={OPCD=14} descr="Add immediate"
    }
}

#Equivalent to... but much more readable than
:powerpc_insn::D_Form lwz mask={OPCD=32} descr="Load word and zero"
```

#### 7.1.5 Operand List Generation

**Automatic Operand Inference**: When an instruction is typed with a form, the operand list is automatically inferred from the form's subfields that have operation types other than `func`.

**Explicit Operand Override**: Instructions can override the automatic operand list by providing an explicit operand specification.

**Inherited Form Operands**: When using inherited forms, operand lists include fields from both parent and child forms that have non-func operation types.

### 7.2 Validation Rules for Instruction Forms

This section defines comprehensive validation rules specific to instruction forms, form inheritance, and typed instructions to ensure correct ISA definitions and prevent conflicts.

#### 7.2.1 Form Definition Validation

**Form Structure Requirements**:
1. **Unique form names**: Form names must be unique within a logic space
2. **Valid subfield definitions**: All subfields must follow standard bitfield syntax and validation rules
3. **Bit range coverage**: Subfield bit ranges must not exceed the logic space size
4. **Operation type validation**: Operation types must be valid (func, target, source, immediate, etc.)
5. **Same-space restriction**: Forms can only be defined within logic spaces

**Validation Rules**:
```
ERROR: Form names must be unique within logic space
ERROR: Subfield bit ranges must be within [0, space_size-1]
ERROR: Invalid operation type in subfield definition
ERROR: Forms can only be defined in logic spaces
WARNING: Subfield ranges overlap within form definition
```

#### 7.2.2 Form Inheritance Validation

**Inheritance Requirements**:
1. **Parent form existence**: Referenced parent forms must exist in the same logic space
2. **Bit range preservation**: Child forms cannot modify inherited field bit ranges
3. **Overlap detection**: New fields in child forms may overlap inherited fields (with warning)
4. **Naming conflicts**: Child form field names must not conflict with inherited field names unless they occupy the same bit range
5. **Circular inheritance prevention**: Forms cannot inherit from themselves directly or indirectly

**Validation Rules**:
```
ERROR: Parent form does not exist in logic space
ERROR: Cannot modify bit range of inherited field
ERROR: Circular form inheritance detected
ERROR: Child form field name conflicts with inherited field (different bit range)
WARNING: Child form field overlaps with inherited field bit range
WARNING: Child form redefines inherited field (same name, same bit range)
```

#### 7.2.3 Typed Instruction Validation

**Type Reference Requirements**:
1. **Form existence**: Referenced forms must exist in the same logic space
2. **Same-space typing**: Instructions can only be typed with forms from their own logic space
3. **Mask field validation**: All mask field names must exist in the instruction's form
4. **Operand consistency**: Explicit operand lists must be consistent with form field definitions
5. **Disambiguation requirements**: Instructions with same mnemonic must have distinguishable masks

**Validation Rules**:
```
ERROR: Form type does not exist in logic space
ERROR: Cross-space form typing not permitted
ERROR: Mask field not found in instruction form
ERROR: Explicit operand references non-existent form field
ERROR: Ambiguous instruction encoding (duplicate mask patterns for same mnemonic)
WARNING: Operand list overrides automatic form inference
WARNING: Mask specification incomplete for disambiguation
```

#### 7.2.4 Logic Space Validation

**Logic Space Requirements**:
1. **Required attributes**: Logic spaces must specify `type=logic` and `size=<bits>`
2. **Invalid attributes**: Logic spaces cannot contain `offset` attributes
3. **Size constraints**: Logic space size must be reasonable (typically 8-64 bits)
4. **Endianness specification**: Optional endianness should be valid (`big` or `little`)

**Validation Rules**:
```
ERROR: Logic space missing required 'size' attribute
ERROR: 'offset' attribute invalid in logic spaces
ERROR: Invalid logic space size (must be 8-64 bits typically)
ERROR: Invalid endianness specification
```

#### 7.2.5 Mask Disambiguation Validation

**Disambiguation Requirements**:
1. **Unique patterns**: Instructions with same mnemonic must have non-overlapping mask patterns
2. **Complete specification**: Critical distinguishing fields should be specified in masks
3. **Field coverage**: Important opcode and extended opcode fields should be masked
4. **Conflict detection**: Tools must detect and report mask pattern conflicts

**Validation Algorithm**:
```
FOR each instruction mnemonic:
  FOR each pair of instructions with same mnemonic:
    IF mask patterns are not mutually exclusive:
      ERROR: Ambiguous instruction encoding
    IF critical distinguishing fields not specified:
      WARNING: Incomplete mask specification
```

#### 7.2.6 Operand Generation Validation

**Operand Inference Requirements**:
1. **Type consistency**: Inferred operands must have consistent operation types
2. **Register file validation**: Register references must target valid register files
3. **Override validation**: Explicit operand lists must reference valid form fields
4. **Order preservation**: Operand order should follow standard conventions

**Validation Rules**:
```
ERROR: Operand references invalid register file
ERROR: Explicit operand not found in form definition
WARNING: Operand order may not follow standard conventions
WARNING: Mixed operand types in instruction definition
```

#### 7.2.7 Complex Validation Scenarios

**Multi-Form Instruction Families**:
- Validate that related instructions (e.g., `add`, `addo`, `addi`) have appropriate form relationships
- Ensure that instruction variants cover expected encoding space without gaps or overlaps
- Check that mask patterns provide sufficient disambiguation for assemblers and disassemblers

**Migration Compatibility**:
- Validate that legacy instruction definitions can coexist with typed instructions
- Ensure that conversion from legacy to typed instructions maintains semantic equivalence
- Check that mixed-mode definitions don't create conflicts

**Cross-Reference Validation**:
- Validate that register file references in forms match defined register spaces
- Ensure that operation types are consistent across related instructions
- Check that instruction semantics are compatible with form definitions

#### 7.2.8 Error Recovery and Suggestions

**Suggested Fixes**:
1. **Form conflicts**: Suggest renaming conflicting forms or merging similar definitions
2. **Mask ambiguity**: Suggest additional mask fields to resolve disambiguation
3. **Inheritance issues**: Suggest creating new forms instead of problematic inheritance
4. **Type mismatches**: Suggest correcting operation types or register file references

**Error Context**:
- Provide line numbers and specific field names in error messages
- Show conflicting definitions side-by-side for comparison
- Suggest alternative approaches for common validation failures

### 7.3 Disassembler Implementation Guidelines

This section provides implementation guidelines for disassemblers, binary analysis tools, and emulators that need to process instruction forms and handle multiple instruction variants with the same mnemonic.

#### 7.3.1 Binary Scanning and Pattern Matching

**Instruction Identification Process**:
1. **Extract instruction word**: Read the appropriate number of bits based on the logic space size
2. **Apply endianness**: Convert the instruction word according to the logic space endianness
3. **Pattern matching**: Compare against mask patterns to identify instruction candidates
4. **Disambiguation**: When multiple instructions match, use additional mask fields to resolve

**Matching Algorithm**:
```
FOR each logic space:
  FOR each instruction in space:
    instruction_word = extract_bits(binary_stream, space.size)
    IF mask_matches(instruction_word, instruction.mask):
      candidates.add(instruction)
  
  resolved_instruction = disambiguate(candidates, instruction_word)
  RETURN resolved_instruction
```

#### 7.3.2 Form-Based Field Extraction

**Field Value Extraction**:
1. **Form resolution**: Determine the instruction's form from the instruction definition
2. **Field mapping**: Use form subfield definitions to extract operand values
3. **Type application**: Apply operation types to interpret extracted values
4. **Register resolution**: Map register indices to register names using register file definitions

**Extraction Process**:
```
form = resolve_form(instruction)
FOR each subfield in form.subfields:
  value = extract_bitfield(instruction_word, subfield.bit_range)
  IF subfield.op_type != "func":
    operands.add(format_operand(value, subfield))
```

#### 7.3.3 Multiple Form Disambiguation

**Disambiguation Strategies**:
1. **Mask priority**: Process instructions with more specific masks first
2. **Form hierarchy**: Consider form inheritance relationships
3. **Conflict resolution**: Handle cases where multiple instructions match the same pattern
4. **Error handling**: Gracefully handle ambiguous or unrecognized instruction patterns

**Priority-Based Matching**:
```
instructions = sort_by_mask_specificity(candidate_instructions)
FOR instruction in instructions:
  IF exact_mask_match(instruction_word, instruction.mask):
    RETURN instruction
```

#### 7.3.4 Operand Generation and Formatting

**Automatic Operand Generation**:
1. **Type-based filtering**: Include only subfields with non-func operation types
2. **Order preservation**: Maintain operand order as defined in form or explicit list
3. **Register file mapping**: Map register indices to symbolic names
4. **Immediate formatting**: Format immediate values according to their type and context

**Override Handling**:
- When instructions provide explicit operand lists, use those instead of form inference
- Validate that explicit operands reference valid form fields
- Maintain operand order as specified in the instruction definition

#### 7.3.5 Assembly Generation

**Mnemonic Construction**:
1. **Base mnemonic**: Use the instruction tag as the base mnemonic
2. **Postfix application**: Append postfixes from subfields with postfix modifiers
3. **Variant indication**: Optionally indicate the form type for disambiguation
4. **Condition flags**: Handle condition code and flag modifications

**Assembly Format**:
```
mnemonic = instruction.tag
FOR subfield in form.subfields:
  IF subfield.has_postfix AND extract_bit(instruction_word, subfield):
    mnemonic += subfield.postfix

assembly = mnemonic + " " + format_operands(operands)
```

#### 7.3.6 Error Handling and Diagnostics

**Unrecognized Instructions**:
- Report unknown instruction patterns with hex encoding
- Suggest potential matches with partial mask compatibility
- Provide context about the logic space and expected instruction formats

**Ambiguous Instructions**:
- Warn when multiple instructions match the same pattern
- Provide details about conflicting mask specifications
- Suggest additional mask fields for disambiguation

**Invalid Encodings**:
- Detect and report invalid bit patterns within instruction fields
- Validate that reserved fields contain expected values
- Handle gracefully when instruction patterns don't match any defined instruction

#### 7.3.7 Performance Optimization

**Lookup Table Generation**:
1. **Precompute masks**: Create lookup tables indexed by instruction patterns
2. **Hierarchical matching**: Organize instructions by primary opcode for faster matching
3. **Conflict pre-detection**: Identify and resolve mask conflicts during initialization
4. **Cache form data**: Precompute form field extraction information

**Optimized Matching**:
```
primary_opcode = extract_bits(instruction_word, 0, 5)  # Common case
candidates = opcode_table[primary_opcode]
IF candidates.length == 1:
  RETURN candidates[0]  # Fast path for unique opcodes
ELSE:
  RETURN detailed_disambiguation(candidates, instruction_word)
```

#### 7.3.8 Integration with Analysis Tools

**Static Analysis Support**:
- Provide instruction type information for control flow analysis
- Export operand type and register usage information
- Support instruction classification (arithmetic, memory, branch, etc.)

**Emulator Integration**:
- Generate execution templates based on form definitions
- Provide operand extraction functions for runtime use
- Support instruction encoding for dynamic code generation

**Debugging Support**:
- Include source line information from ISA definitions
- Provide detailed instruction breakdown with field-by-field analysis
- Support instruction variant comparison and analysis

#### 7.3.9 Example Disassembler Implementation

**PowerPC ADD Instruction Family**:
```
Binary: 0x7C632214
Primary opcode: 31 (bits 0-5)
Extended opcode: 266 (bits 21-30)
Rc bit: 0 (bit 31)

Candidates:
- :powerpc_insn::X_Form add mask={OPCD=31, XO=266, Rc=0}
- :powerpc_insn::XO_Form addo mask={OPCD=31, XO=266, OE=1, Rc=0}

OE bit check: 0 (bit 21)
Selected: X_Form add
Operands: r3, r3, r4 (extracted from RT, RA, RB fields)
Assembly: "add r3, r3, r4"
```

**Complex Disambiguation Example**:
```
Binary: 0x80030000
Primary opcode: 32 (bits 0-5)

Match: :powerpc_insn::D_Form lwz mask={OPCD=32}
Form: D_Form
RT: 0 (bits 6-10) -> r0
RA: 3 (bits 11-15) -> r3  
D: 0 (bits 16-31) -> 0
Assembly: "lwz r0, 0(r3)"
```

## 8. Memory Mapped Connections Between Memory Spaces (`:bus`)

Addressing within a memory space typically always starts at 0x0 and ends at the size of the space. However this leads to overlaps between spaces. The bus allows setup of `bus address ranges` which point to different memory spaces.

- **Syntax**: `:bus <bus_tag> addr=<bits> ranges={ range definitions }`
- **Options**:
  - `bus_tag`: Defines the tag to access the bus by name, must be a **single_word**
  - `addr=<bits>`: Defines address size, must be a valid **numeric_literal**
  - `ranges={ range definitions }`: Defines a series of addresses mapping a named `<space_tag>`

- **Range Definitions**:
  - List of `[<bus_range>]->[<space_tag>] [prio=<numeric_literal>] [space_off=<numeric_literal>]` definitions on separate lines
  - **REQUIRED** `bus_range` must be a combination of valid **numeric_literal**, **range operaters** [`+`,`--`], or **size units** [`kB`,`MB`,`GB`,`TB`]. The start and end must within the defined address size of the bus.  
    -`<start>+<size>`: The **range operator** `+` indicates that the range is a <address>+<size> with the size being any valid **numeric literal** and an optional **size unit**; if a size unit is not provided the default unit size shall be "bytes". 
    -`<start>--<inclusive end>`: The **range operator** '--' indicates that the range is a <address>--<inclusive end> with the inclusive end being any valid **numeric literal** without size units.    
  - **REQUIRED** `space_tag` must be a previously defined `space_tag`. Each tag should be colored per the previously assigned `space_tag` color
  - **OPTIONAL** `prio=<numeric_literal>`: must be a valid numeric_literal and defines relative priority on any overlapping ranges. A larger lower priority ranges could have holes punched in it with higher priority ranges taking over specific sub ranges
  - **OPTIONAL** `space_off=<numeric_literal>`: must be a valid numeric_literal and defines the starting offset inside the space for this bus definition. If not provided will default to 0

- **Example**:
  ```plaintext
  :space small_flash addr=32 word=32 type=ro align=12 endian=big
  :space large_flash addr=32 word=32 type=ro align=12 endian=big
  :space ram addr=32 word=32 type=rw align=16 endian=big
  :space etpu addr=16 word=24 type=memio align=16 endian=big

  :bus sysbus addr=32 ranges={
      0x0 -- 0x40000       -> small_flash
      0x800000   +8MB      -> large_flash
      0x40000000 +512kB    -> ram 
      0x40000400 +1kB      -> small_flash offset=0x1080 prio=1 # flash image in ram space
      0xC3F80000 +0x10000  -> etpu #64kB equivalent in bytes (no size units) 
  }
  ```

## 9. Declarations Inside a Memory Space

After defining a memory space, you can use the space name as a command to define `fields` or `instructions` within that space using the syntax `:<space_tag> <field_tag|instruction_tag>`.

### 9.1 Field Definition (`:<space_tag> <field_tag>`)

`field_tag` must be a `single_word` (e.g., `GPR`, `XER`, `CR`) and will be used as the `field_name`. For indexed fields using bracket notation, `field_name` shall be `<field_tag><index>` where the index ranges from startindex to endindex. `field_tag` needs to be colored the same as the encompassing `space_tag`.

#### 9.1.1 Syntax Forms

There are several ways to define fields:

**New Field Definition**:
```
:<space_tag> <field_tag>[<start_index>-<end_index>] [offset=<numeric_literal>] [size=<bits>] [reset=<value>] [descr="<description>"] [subfields={list of subfield definitions}]
```

or

```
:<space_tag> <field_tag> [offset=<numeric_literal>] [size=<bits>] [reset=<value>] [descr="<description>"] [subfields={list of subfield definitions}]
```

**New Field Options**:
- **OPTIONAL** `[<start_index>-<end_index>]`: Index range for register arrays using bracket notation. Both indices must be valid numeric literals. `start_index` must be ≥ 0, `end_index` must be ≥ `start_index`, and the total count (`end_index - start_index + 1`) must be ≤ 65535.
- **OPTIONAL** `offset=<numeric_literal>`: Base offset within the memory space. Must be valid numeric literal that fits within an address of the defined space. If not provided shall start just after the previously defined field. Offsets can overlap previously defined field ranges however a warning shall be provided.
- **OPTIONAL** `size=<numeric_literal>`: Total size in bits. Must be > 0 and ≤ 512 bits. Must be valid numeric literal. Defaults to `word` size of the parent space.
- **OPTIONAL** `reset=<numeric_literal>`: Reset value (default 0 if not provided). Must be valid numeric literal. Default = 0.
- **OPTIONAL** `descr="<description>"`: Textual description.

**Redirect Definition**:
```
:<space_tag> <field_tag> [redirect=<context_reference>] [descr="<description>"] [subfields={list of subfield definitions}]
```

Redirects take on the offset and size of the other field_tag or subfield referenced in the redirect option_tag.

**Redirect Field Options**:
- **REQUIRED** `redirect=<context_reference>`: References a previously defined field using context operator syntax (e.g., `field::subfield` or `$space::field::subfield`). This creates a new `field_name` that maps to the same memory offset and bits.
- **OPTIONAL** `descr="<description>"`: Textual description.

**Appending Subfield Definitions**:
```
:<space_tag> <previously defined field_tag> [subfields={list of subfield definitions}]
```

Appending subfield definitions can happen any time after initial field_tag definition.

**Untagged Subfield Definitions**:
```
:<space_tag> [size=<numeric_literal>] [subfields={list of subfields}]
```

Subfields defined in untagged fields do not have a memory space and can be referenced elsewhere.

**Untagged Subfield Options**:
- **OPTIONAL** `size=<numeric_literal>`: Total size in bits. Must be > 0 and ≤ 512 bits. Must be valid numeric literal. Defaults to `word` size of the parent space.
- **REQUIRED** `subfields={list of subfields}` see Section 7.1.2 below

#### 9.1.2 Subfields

Each subfield definition shall occur within a `subfields={}` option tag context window. Only one subfield definition shall be on a line and following the following format:

**Syntax**: `<subfield_tag> @(<bit_spec>)[|<bit_spec>...] [op=<type>[.<subtype>][|<type>...]] [descr="<description>"]`

**Subfield Components**:
- **REQUIRED** `<subfield_tag>`: Unique name for the subfield (e.g., `AA`, `BD`, `rA`). `subfield_tag` shall be highlighted/colored the same as the encompassing `space_tag`.
- **REQUIRED** `@(<bit_field>)`: Bit specification for the field within a field (see "Bit Specification Details" in Section 8).
  - Example: `DCRN @(16-20|11-15)` means bits 16-20 are concatenated with bits 11-15 to form the `DCRN` field.
- **OPTIONAL** `op=<type>[.<subtype>][|<type>...]`: Defines the operational type and properties of the field. Multiple types can be OR'd using `|`.
  - `imm`: Immediate values are right shifted (e.g., `@(16-19)`=0x0000F000 will be right shifted to display 0xF).
  - `ident`: Immediate value represents a field identifier (may be operation specific).
  - `<space_tag>`: Field accesses another space somehow
    - `<space_tag>.<field_tag>`: Field identifies or accesses another field in another space somehow. Example: this is an instruction that accesses registers in a register file GPR by id (value of 1 access GPR1, value of 5 accesses GPR5, etc.).
    - `<space_tag>.SPR`: Field by index into the SPR field_tag (example subtype).
  - `addr`: Field is an address.
  - `source`: Field is a source operand, mutually exclusive with `target`.
  - `target`: Field is a target operand, mutually exclusive with `source`.
  - `func`: Field is part of the functional opcode (distinguishes instructions).
- **OPTIONAL** `descr="<description>"`: Textual description of the field.

#### 9.1.3 Field Validation Rules

- **Simple Types**: All simple types must have a valid format and value according to the simple type.
- **Redirect Mutual Exclusivity**: `redirect` cannot be used with `offset` or `size` as it will take on the `size` and `offset` of the redirect 
- **Index Range Validation**: When using bracket notation, `start_index` ≤ `end_index`, both must be ≥ 0, and the total count (`end_index - start_index + 1`) must be ≤ 65535
- **Mutually Exclusive Attributes**: Bracket notation cannot be used with deprecated `count=` or `name=` attributes
- **Field Name Tracking**: Generated field_names (from bracket notation or field_tag if no bracket notation provided) are tracked for later redirect validation or access.
- **Size Limit**: `size` must be ≤ 512 bits and > 0 bits
- **Range Validation**: The start and end offset shall be tracked to check for overlaps. Redirects can overlap without warning but new fields shall generate a warning if they overlap.
- **Bitfield Numbering**: Bit indices shall be in the range 0..size-1 of the field definition with 0 being the most significant bit and size-1 being the least significant.

#### 9.1.4 Field Examples

```plaintext
:space reg addr=32 word=64 type=register

# Simple register definition
:reg PC size=64 offset=0x0 reset=0x0

# Register file with bracket notation
:reg GPR[0-31] offset=0x100 size=64 reset=0
# This creates: GPR0, GPR1, GPR2, ..., GPR31

# Index range starting from non-zero
:reg SPR[256-511] offset=0x1000 size=32

# Hex indices for special register ranges
:reg MSR[0x0-0xF] offset=0x2000 size=64

# Register redirect (mutually exclusive with other options except description)
:reg SP redirect=GPR1
:reg SP2 redirect=GPR2 descr="Special Purpose 2"

# Declaring subfields
:reg XER offset=0x200 size=32 reset=0x0 subfields={
    SO @(0) descr="Summary Overflow"
    OV @(1) descr="Overflow"
    CA @(2) descr="Carry"
}

:space insn addr=32 word=32 type=rw

# Untagged subfield definitions for instructions
:insn subfields={
    AA @(30) op=func descr="Absolute Address flag, bit 30"
    BD @(16-29|0b00) op=imm descr="Displacement, bits 16-29, padded with 00b"
    rA @(11-15) op=reg.GPR descr="Register A, bits 11-15, is a GPR"
    opc6 @(0-5) op=func descr="Primary 6-bit opcode field, bits 0-5"
}

:insn size=16 subfields={
    AA16 @(14) op=func # inline comment
    # error_bitidx @(20) # should provide error because maximum bit index is 15 in this space
}
```

### 9.2 Instruction Definition (`:<space_tag> <instruction_tag>`)

Defines individual machine instructions, their mnemonics, operand fields, and matching criteria (mask).

**Syntax**: 
- Basic form: `:<space_tag> <instruction_tag> (<field1>,<field2>,...) [mask={<MaskSpecification>}] [descr="<description>"] [semantics={ <SemanticsBlock> }]`
- Typed form: `:<space_tag>::<form_name> <instruction_tag> [(<field1>,<field2>,...)] [mask={<MaskSpecification>}] [descr="<description>"] [semantics={ <SemanticsBlock> }]`

**Attributes**:
- `<instruction_tag>`: The assembly mnemonic for the instruction (e.g., `add`, `b`, `cmpi`).
- `(<field1>,<field2>,@(bit_field)...)`: Comma-separated list of fields (defined earlier in an untagged field definition of the same size) or anonymous bit fields that this instruction uses as operands for its operation.
- `mask={<MaskSpecification>}`: Defines the fixed bit patterns used to identify this instruction.
  - **For basic instructions**: The specification is a set of `name=value` or `@(bit_range)=value` pairs separated by spaces or new lines.
  - **For typed instructions**: The specification references field names from the instruction's form definition.
  - `name` refers to a field defined in subfields (basic) or form fields (typed).
  - `@(bit_field)` refers to an anonymous bit field (see "Bit Specification Details").
  - `value` can be binary (e.g., `0b011111`), decimal (e.g., `0`), or hexadecimal (e.g., `0x1F`). This indicates the expected immediate value of the bitfield and is used to match bit patterns to differentiate instructions when disassembling.
  - Multiple mask entries are effectively ANDed together. They can be on the same line separated by spaces, or on multiple lines within the `{}`.
  - **Form disambiguation**: When multiple instructions share the same mnemonic but use different forms, masks must provide sufficient discrimination to uniquely identify each instruction variant.
- `descr="<description>"`: Textual description of the instruction.
- `semantics={ <SemanticsBlock> }`: (Future Use) A block intended for Register Transfer Language (RTL) or other semantic descriptions for emulation. Currently not fully parsed/utilized.

#### 9.2.1 Mask Disambiguation for Instruction Forms

When multiple instruction variants share the same mnemonic but use different forms, the mask specifications must provide sufficient discrimination to uniquely identify each variant during disassembly and analysis.

**Disambiguation Requirements**:
1. **Unique identification**: Each instruction variant must have a unique combination of mask field values
2. **Form field validation**: Mask field names must exist in the instruction's associated form
3. **Completeness**: Critical distinguishing fields (like opcodes and extended opcodes) should be specified
4. **Inheritance consideration**: When using inherited forms, masks can reference fields from both parent and child forms

**Disambiguation Examples**:
```isa
# Same mnemonic with different forms requiring different mask patterns
:powerpc_insn::X_Form add mask={OPCD=31, XO=266, Rc=0} descr="Add (X-Form)"
:powerpc_insn::XO_Form addo mask={OPCD=31, XO=266, OE=1, Rc=0} descr="Add with overflow (XO-Form)"
:powerpc_insn::D_Form addi mask={OPCD=14} descr="Add immediate (D-Form)"

# Load instructions with different addressing modes
:powerpc_insn::D_Form lwz mask={OPCD=32} descr="Load word displacement"
:powerpc_insn::X_Form lwzx mask={OPCD=31, XO=23, Rc=0} descr="Load word indexed"
```

**Validation Rules**:
- Mask field names must exist in the referenced form's subfield definitions
- Each instruction variant with the same mnemonic must have distinguishable mask patterns
- Tools should validate that mask combinations don't create ambiguous instruction encodings

**Example**:
```plaintext
:space insn word=32 addr=32 type=rw align=16 endian=big
:space reg word=64 addr=32 type=register align=64 endian=big

:reg GPR count=32 name=r%d

:insn size=32 subfields={
    opc6 @(0-5) op=func
    rD @(6-10) op=target|reg.GPR
    rA @(11-15) op=source|reg.GPR
    rB @(16-20) op=source|reg.GPR
    OE @(21) op=func
    Rc @(31) op=func
}

:insn add (rD,rA,rB) mask={opc6=0b011111 OE=0 @(22-30)=0b100001010 Rc=0} descr="Add"
    semantics={ rD = rA+rB }
:insn addi (rD,rA,SIMM) mask={opc6=0b001110} descr="Add Immediate"
:insn b (LI,AA,LK) mask={opc6=0b010010} descr="Branch"
```

## 10. Core File Specifics (`.core`)
### 10.1 Include Directive (`:include`)
This file adds a command `:include` which will point to an `.isa` or `.isaext` file elsewhere in the filesystem and include their contexts in the root context per the isa standard.   Linting this file is where missing symbols in `.isaext` should or symbol conflicts should be validated.

## 11. System File Specifics (`.sys`)
### 11.1 Attach Directive (`:attach`)
`:attach <context-tag> <filepath>`

## 12. Glossary

- **Context Window**: A section of the file that begins with a directive and continues until the next directive
- **Space Tag**: A unique identifier for a memory space (e.g., `ram`, `reg`)
- **Field Tag**: A unique identifier for a field within a space
- **Subcontext**: A nested section within a context window, delimited by `{}` or `()`
- **Bit Field**: A specification of which bits within a container are used for a field, `@()`
- **Numeric Literal**: A number specified in decimal, hexadecimal, binary, or octal format
- **Index Range**: A pair of numbers defining a range in brackets `[]`
- **Address Range**: A pair of numbers with a size (and optional size units) or inclusive end address defined by a range operator `+` or `--`. 
