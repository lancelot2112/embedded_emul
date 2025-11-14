# Instruction Forms Addendum

## Overview
This addendum describes the addition of instruction forms to the ISA language specification. Instruction forms provide templating capabilities for common instruction encodings, reducing duplication and improving maintainability when defining complex instruction sets using a typing approach.

## New Space Type: `logic`

### Syntax
```isa
:space <space_name> type=logic [size=<bits>] [endian=<endianness>]
```

### Characteristics
- **Purpose**: Defines instruction forms and instruction definitions for binary analysis and encoding
- **No bus mapping**: Logic spaces are not added to buses and have no physical memory representation
- **Binary scanning**: Forms and instructions defined in logic spaces are used to scan and interpret binary instruction encodings
- **Invalid tags**: The `offset` tag is invalid in logic spaces since there is no concept of memory mapping
- **Unified space**: Both forms and instructions coexist in the same logic space

### Example
```isa
:space powerpc_insn type=logic size=32 endian=big
```

## Form Definition (Typing)

### Syntax
Forms are defined as fields with subfields within a logic space:

```isa
:space_name form_name subfields={
    field_name @(bit_range) op=<operation_type> [descr="<description>"]
    ...
}
```

### Form Characteristics
- **Type templates**: Forms act as types that instructions can be typed with
- **Bitfield definitions**: Use standard bitfield syntax for encoding layouts
- **Operation types**: Standard operation types (func, target, source, immediate, etc.)
- **Documentation**: Support description strings for form documentation
- **Same-space only**: Forms can only be referenced within the same logic space

### Example Form Definitions
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

:powerpc_insn I_Form subfields={
    OPCD @(0-5) op=func descr="Primary opcode"
    LI @(6-29) op=immediate descr="Immediate value"
    AA @(30) op=func descr="Absolute address"
    LK @(31) op=func descr="Link bit"
}

:powerpc_insn D_Form subfields={
    OPCD @(0-5) op=func descr="Primary opcode"
    RT @(6-10) op=target|reg.GPR descr="Target register"
    RA @(11-15) op=source|reg.GPR descr="Source register"
    D @(16-31) op=immediate descr="Displacement"
}
```

## Typed Instructions

### Syntax
Instructions are typed using the context operator with forms:

```isa
:space_name;form_name instruction_name [operand_list] [mask={<mask_specification>}] [descr="<description>"] [semantics={<semantics_block>}]
```

### Characteristics
- **Type specification**: Instructions declare their encoding type using `;form_name`
- **Same-space typing**: Form references are local to the logic space (no cross-space typing)
- **Automatic operand inference**: Operand list inferred from form fields with non-func operation types
- **Explicit operand override**: Instructions can provide explicit operand lists when needed

### Example Typed Instructions
```isa
:space powerpc_insn type=logic size=32 endian=big

# X_Form typed instructions
:powerpc_insn::X_Form add mask={OPCD=31, XO=266, Rc=0} descr="Add"
:powerpc_insn::X_Form add. mask={OPCD=31, XO=266, Rc=1} descr="Add and record"
:powerpc_insn::X_Form sub mask={OPCD=31, XO=40, Rc=0} descr="Subtract"
:powerpc_insn::X_Form mullw mask={OPCD=31, XO=235, Rc=0} descr="Multiply low word"

# Override operand list when needed (mr uses only RT, RA despite X_Form having RT, RA, RB)
:powerpc_insn::X_Form mr (RT, RA) mask={OPCD=31, XO=444, RB=0, Rc=0} descr="Move register"

# I_Form typed instructions  
:powerpc_insn::I_Form b mask={OPCD=18, AA=0, LK=0} descr="Branch"
:powerpc_insn::I_Form ba mask={OPCD=18, AA=1, LK=0} descr="Branch absolute"
:powerpc_insn::I_Form bl mask={OPCD=18, AA=0, LK=1} descr="Branch and link"

# D_Form typed instructions
:powerpc_insn::D_Form lwz mask={OPCD=32} descr="Load word and zero"
:powerpc_insn::D_Form stw mask={OPCD=36} descr="Store word"
:powerpc_insn::D_Form addi mask={OPCD=14} descr="Add immediate"
```

## Form Inheritance

### Syntax
Forms can inherit from other forms within the same logic space:

```isa
:space_name;parent_form child_form subfields={
    additional_field @(bit_range) op=<operation_type> [descr="<description>"]
    ...
}
```

### Inheritance Rules
1. **Preserve bit ranges**: Inherited fields maintain their original bit range declarations
2. **Allow overlaps**: Child form fields may overlap with inherited fields
3. **Overlap warnings**: Language tools should warn when new fields overlap with inherited fields
4. **No greedy changes**: Bit ranges of inherited fields cannot be modified
5. **Force new forms**: For bit range modifications, create a new form rather than inheriting

### Example Form Inheritance
```isa
:space powerpc_insn type=logic size=32 endian=big

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
    # XO remains @(21-30), OE is @(21)
}

# Instructions using the inherited form
:powerpc_insn::XO_Form addo mask={OPCD=31, XO=266, OE=1, Rc=0} descr="Add with overflow"
:powerpc_insn::XO_Form subfo mask={OPCD=31, XO=40, OE=1, Rc=0} descr="Subtract from with overflow"

# For bit range changes, create a new form instead of inheriting
:powerpc_insn XO_Alt_Form subfields={
    OPCD @(0-5) op=func
    RT @(6-10) op=target|reg.GPR
    RA @(11-15) op=source|reg.GPR  
    RB @(16-20) op=source|reg.GPR
    OE @(21) op=func descr="Overflow enable"
    XO @(22-30) op=func descr="Extended opcode (shifted)"
    Rc @(31) op=func
}
```

## Operand List Generation

### Automatic Operand Inference
When an instruction is typed with a form, the operand list is automatically inferred from the form's subfields that have operation types other than `func`:

```isa
# This form definition:
:powerpc_insn X_Form subfields={
    OPCD @(0-5) op=func
    RT @(6-10) op=target|reg.GPR  
    RA @(11-15) op=source|reg.GPR
    RB @(16-20) op=source|reg.GPR
    XO @(21-30) op=func
    Rc @(31) op=func
}

# Automatically generates operand list: (RT, RA, RB)
:powerpc_insn::X_Form add mask={OPCD=31, XO=266, Rc=0}
```

### Explicit Operand Override
Instructions can override the automatic operand list by providing an explicit operand specification:

```isa
# Override to use only two operands instead of three
:powerpc_insn::X_Form mr (RT, RA) mask={OPCD=31, XO=444, RB=0, Rc=0} descr="Move register"
```

### Inherited Form Operands
When using inherited forms, operand lists include fields from both parent and child forms:

```isa
# XO_Form inherits X_Form fields, adds OE field
:powerpc_insn::X_Form XO_Form subfields={
    OE @(21) op=func  # func type, not included in operands
}

# Operand list still (RT, RA, RB) - same as parent X_Form
:powerpc_insn::XO_Form addo mask={OPCD=31, XO=266, OE=1}
```

## Binary Scanning Applications

### Purpose
Forms and instructions defined in logic spaces enable:

1. **Instruction decoding**: Pattern matching against binary instruction streams
2. **Disassembly**: Converting binary encodings back to assembly mnemonics
3. **Static analysis**: Understanding instruction structure for analysis tools
4. **Emulation**: Providing encoding templates for instruction execution

### Scanning Process
1. **Pattern matching**: Use mask specifications to identify instruction types
2. **Field extraction**: Extract operand values using form bitfield definitions
3. **Assembly generation**: Map extracted values to assembly syntax using operand lists

## Language Grammar Extensions

### New Productions

```ebnf
space_declaration ::= ':space' identifier 'type=' space_type [space_attributes]

space_type ::= 'insn' | 'reg' | 'bus' | 'logic'

logic_space_attributes ::= 'size=' integer ['endian=' endianness]
                          # Note: 'offset' is invalid for logic spaces

form_definition ::= ':' space_name form_name 'subfields=' '{' subfield_list '}'

form_inheritance ::= ':' space_name ';' parent_form child_form 'subfields=' '{' subfield_list '}'

typed_instruction ::= ':' space_name ';' form_name instruction_name 
                     [operand_list] 
                     [mask_specification] 
                     [instruction_attributes]
```

### Modified Productions

```ebnf
instruction_definition ::= instruction_basic | typed_instruction

mask_specification ::= 'mask=' '{' mask_constraint_list '}'

mask_constraint ::= field_name '=' (integer | binary_literal | hex_literal)
```

## Implementation Requirements

### Language Server Extensions
1. **Form type validation**: Ensure form type references resolve to valid forms within the same logic space
2. **Same-space enforcement**: Reject cross-space form typing attempts
3. **Operand inference**: Automatically generate operand lists from form fields
4. **Mask validation**: Verify mask constraints reference valid form fields
5. **Inheritance validation**: Check parent form references and detect bit range overlaps

### Semantic Analysis
1. **Form compatibility**: Ensure instruction masks are compatible with typed forms
2. **Bitfield coverage**: Validate that form masks provide sufficient instruction discrimination
3. **Operand type checking**: Verify operand types match form field operation types
4. **Overlap detection**: Generate warnings for overlapping fields in inherited forms

### Error Handling
1. **Invalid form types**: Report errors for non-existent form types
2. **Cross-space typing**: Reject attempts to type with forms from other spaces
3. **Logic space constraints**: Reject `offset` attributes in logic spaces
4. **Mask field validation**: Report errors for mask fields not present in typed forms
5. **Inheritance errors**: Report errors for invalid parent form references

## Migration Strategy

### Backward Compatibility
Existing instruction definitions without form typing continue to work unchanged:

```isa
# Existing syntax remains valid in logic spaces
:powerpc_insn add (RT, RA, RB) mask={@(0-31)=0x7C000214}
```

### Gradual Adoption
ISA definitions can incrementally adopt instruction form typing:

1. **Phase 1**: Convert existing instruction spaces to logic spaces
2. **Phase 2**: Define common forms for high-frequency instruction patterns
3. **Phase 3**: Convert instructions to use form typing
4. **Phase 4**: Leverage inheritance for complex instruction variants

### Tool Support
Development tools should support both syntaxes during migration period, with optional warnings or suggestions to adopt form-based definitions for improved maintainability.

## Example: Complete PowerPC Logic Space

```isa
:space powerpc_insn type=logic size=32 endian=big

# Form definitions
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

# Inherited form with overlap warning
:powerpc_insn::X_Form XO_Form subfields={
    OE @(21) op=func descr="Overflow enable"  # Overlaps with XO @(21-30)
}

# Typed instructions
:powerpc_insn::X_Form add mask={OPCD=31, XO=266, Rc=0} descr="Add"
:powerpc_insn::X_Form mr (RT, RA) mask={OPCD=31, XO=444, RB=0, Rc=0} descr="Move register"
:powerpc_insn::XO_Form addo mask={OPCD=31, XO=266, OE=1, Rc=0} descr="Add with overflow"
:powerpc_insn::D_Form lwz mask={OPCD=32} descr="Load word and zero"

# Mixed approach during migration
:powerpc_insn legacy_add (RT, RA, RB) mask={@(0-31)=0x7C000214} descr="Legacy definition"
```