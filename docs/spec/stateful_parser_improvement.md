# Stateful Parser Improvement Specification

## Overview

This specification outlines the refactoring of the field parsing context detection system from a string-based approach to a tokenizer-based state machine approach. This improvement addresses critical issues with multiline declarations, context ambiguity, and parsing robustness.

## Current Problems Analysis

### String-based Context Detection Issues

**1. Fragile Pattern Matching:**
- Uses regex patterns and string searching (`includes()`, `test()`) which are brittle
- Manual brace/parentheses counting that fails on nested structures  
- Multiple disconnected `if` statements that can trigger multiple contexts simultaneously
- Fails on multiline declarations where string context gets truncated
- No proper state transitions - each method independently determines context

**2. Specific Problem Methods:**
- `isTokenInMaskContext()` - uses regex + manual brace counting
- `isTokenInInstructionOperandList()` - uses regex + manual paren counting  
- `isTokenInSubfieldsContext()` - uses regex + manual brace counting
- `analyzeTokenContext()` - disconnected if-else chain that misses edge cases

**3. Observable Issues:**
- Field options incorrectly flagged in instruction operand lists
- Context detection fails across line boundaries
- Nested structures cause incorrect brace/paren counting
- Multiple validation contexts can be detected simultaneously

## Proposed State Machine Architecture

### Core Concept

Replace string-based parsing with a stateful token parser that tracks context as it processes tokens sequentially, maintaining proper state transitions and handling complex nested structures.

### State Machine Design

#### 1. Context States

```typescript
enum ContextState {
  UNKNOWN = 'unknown',
  FIELD_DEFINITION = 'field-definition',
  FIELD_OPTIONS = 'field-options', 
  INSTRUCTION_DEFINITION = 'instruction-definition',
  INSTRUCTION_OPERANDS = 'instruction-operands',
  INSTRUCTION_OPTIONS = 'instruction-options',
  MASK_DEFINITION = 'mask-definition',
  SUBFIELDS_DEFINITION = 'subfields-definition',
  SUBFIELD_OPTIONS = 'subfield-options',
  RANGES_DEFINITION = 'ranges-definition'
}
```

#### 2. State Transition Triggers

```typescript
enum TransitionTrigger {
  DIRECTIVE_TOKEN,      // :reg, :insn, etc.
  IDENTIFIER_TOKEN,     // field names, instruction names
  OPEN_PAREN,          // (
  CLOSE_PAREN,         // )
  OPEN_BRACE,          // {
  CLOSE_BRACE,         // }
  EQUALS_SIGN,         // =
  MASK_KEYWORD,        // 'mask'
  SUBFIELDS_KEYWORD,   // 'subfields'
  RANGES_KEYWORD,      // 'ranges'
  BIT_FIELD_TOKEN,     // @(...)
  NEWLINE_OR_END
}
```

#### 3. State Machine Class Structure

```typescript
class TokenContextStateMachine {
  private currentState: ContextState = ContextState.UNKNOWN;
  private stateStack: ContextState[] = [];
  private braceDepth: number = 0;
  private parenDepth: number = 0;
  private lastDirective: string | null = null;
  private tokenHistory: Token[] = [];
  
  // Main processing method
  public processToken(token: Token): ContextState {
    this.tokenHistory.push(token);
    const trigger = this.getTransitionTrigger(token);
    this.currentState = this.transition(this.currentState, trigger, token);
    this.updateDepthCounters(token);
    return this.currentState;
  }
  
  // State transition logic
  private transition(from: ContextState, trigger: TransitionTrigger, token: Token): ContextState {
    // Comprehensive transition table implementation
  }
  
  // Helper methods
  private getTransitionTrigger(token: Token): TransitionTrigger;
  private updateDepthCounters(token: Token): void;
  private pushState(state: ContextState): void;
  private popState(): ContextState;
  private shouldValidateAsFieldOption(): boolean;
  private shouldValidateAsSubfieldOption(): boolean;
  private shouldValidateAsInstructionOption(): boolean;
}
```

#### 4. State Transition Table

The state machine will use a comprehensive transition table to handle all valid state changes:

```typescript
private static readonly TRANSITION_TABLE: Map<string, ContextState> = new Map([
  // Format: "currentState|trigger" -> nextState
  ["UNKNOWN|DIRECTIVE_TOKEN", "FIELD_DEFINITION"],
  ["FIELD_DEFINITION|IDENTIFIER_TOKEN", "FIELD_OPTIONS"],
  ["FIELD_DEFINITION|OPEN_PAREN", "INSTRUCTION_OPERANDS"],
  ["INSTRUCTION_OPERANDS|CLOSE_PAREN", "INSTRUCTION_OPTIONS"],
  ["INSTRUCTION_OPTIONS|MASK_KEYWORD", "MASK_DEFINITION"],
  ["MASK_DEFINITION|OPEN_BRACE", "MASK_DEFINITION"],
  ["MASK_DEFINITION|CLOSE_BRACE", "INSTRUCTION_OPTIONS"],
  // ... comprehensive transition mappings
]);
```

## Integration Strategy

### Phase 1: Create State Machine Infrastructure

1. **Implement `TokenContextStateMachine` class**
   - Define all state transitions in transition table
   - Handle nesting with state stack for complex structures
   - Implement proper depth counting for braces/parentheses

2. **Add State Machine to Semantic Analyzer**
   - Initialize state machine per document analysis
   - Process tokens sequentially through state machine
   - Maintain context state throughout analysis

### Phase 2: Replace String-based Methods

1. **Remove Existing String Methods**
   - Delete `isTokenInMaskContext()`
   - Delete `isTokenInInstructionOperandList()`
   - Delete `isTokenInSubfieldsContext()`
   - Simplify `analyzeTokenContext()`

2. **Integrate State Machine Context**
   - Replace string analysis with state machine queries
   - Use sequential token processing instead of string analysis
   - Leverage proper state transitions for validation decisions

### Phase 3: Enhanced Multiline Support

1. **Context Persistence**
   - State machine naturally handles multiline declarations
   - Maintain context across line boundaries
   - Proper handling of incomplete/partial declarations

2. **Robust Nesting**
   - Proper tracking of nested braces and parentheses
   - State stack for handling complex nested structures
   - Recovery mechanisms for malformed syntax

## Implementation Benefits

### Accuracy Improvements
- **No Regex Fragility:** Eliminates brittle pattern matching
- **Proper Nesting:** Correct handling of nested structures
- **Context Isolation:** Prevents multiple context detection
- **Token-based Logic:** Leverages existing robust tokenizer

### Robustness Enhancements
- **Multiline Support:** Natural handling of declarations spanning lines
- **State Consistency:** Proper state transitions prevent edge cases
- **Error Recovery:** Graceful handling of malformed syntax
- **Extensibility:** Easy addition of new contexts and rules

### Maintainability Benefits
- **Clear Logic:** State machine provides explicit transition rules
- **Centralized Control:** All context logic in one place
- **Testable Components:** Individual state transitions can be unit tested
- **Documentation:** State diagrams provide clear behavior specification

## Implementation Steps

1. **Analysis Phase**
   - Document all current edge cases and failure modes
   - Map existing string-based logic to state transitions
   - Identify all required states and triggers

2. **Design Phase**
   - Create comprehensive state transition diagram
   - Define transition table with all valid state changes
   - Design error recovery and edge case handling

3. **Implementation Phase**
   - Implement `TokenContextStateMachine` class
   - Create comprehensive unit tests for state transitions
   - Integrate state machine into semantic analyzer

4. **Validation Phase**
   - Test against all existing example files
   - Verify multiline declaration handling
   - Validate context detection accuracy

5. **Integration Phase**
   - Replace string-based methods with state machine calls
   - Update semantic analyzer to use sequential processing
   - Ensure backward compatibility with existing functionality

## Success Criteria

1. **Functional Requirements**
   - All existing validation functionality preserved
   - Correct context detection for multiline declarations
   - Proper handling of nested structures
   - No false positive context detection

2. **Quality Requirements**
   - 100% unit test coverage for state transitions
   - Performance equal to or better than current implementation
   - Clear, maintainable code structure
   - Comprehensive documentation of state behavior

3. **Robustness Requirements**
   - Graceful handling of malformed syntax
   - Consistent behavior across all ISA language constructs
   - Extensible design for future language features

## Conclusion

This stateful parser improvement will eliminate the fundamental issues with string-based context detection, providing a robust, maintainable, and extensible foundation for ISA language parsing. The state machine approach aligns with established parsing best practices and leverages the existing tokenizer infrastructure for optimal performance and reliability.