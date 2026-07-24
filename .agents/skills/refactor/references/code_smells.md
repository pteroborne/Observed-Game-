# Code Smells Taxonomy & Reference Guide

Based on [Refactoring.Guru Code Smells Catalog](https://refactoring.guru/refactoring/smells).

Code smells are key indicators of poor design or accumulating technical debt. They do not cause immediate runtime errors, but slow down development, increase bug frequency, and impair system maintainability.

---

## 1. Bloaters

Bloaters are code, methods, structs, or classes that have grown to such immense proportions that they are difficult to read, understand, or modify.

### 1.1 Long Method
- **Symptoms**: A method or function containing too many lines of code (typically > 30-50 lines).
- **Causes**: Continual addition of functionality without extracting helper functions.
- **Treatment**:
  - `Extract Method`: Move cohesive code blocks into named helper functions.
  - `Reduce Temp Variables`: Use `Replace Temp with Query` or `Inline Temp`.
  - `Decompose Conditional`: Extract conditional branches into descriptive helper functions.

### 1.2 Large Class / Oversized Module
- **Symptoms**: A class/struct/module with too many fields, methods, or responsibilities.
- **Causes**: Violating the Single Responsibility Principle (SRP); trying to make one object handle everything.
- **Treatment**:
  - `Extract Class / Struct`: Group related fields and behaviors into new sub-structures.
  - `Extract Interface / Trait`: Define focused traits for specific interfaces.

### 1.3 Primitive Obsession
- **Symptoms**: Using primitive types (`u32`, `i32`, `String`, `bool`) for domain concepts (e.g., currency, IDs, ranges, coordinates).
- **Causes**: Quick prototyping where formal types weren't initially defined.
- **Treatment**:
  - `Replace Primitive with Value Object`: Wrap primitives in newtypes (e.g., `struct RoomId(u32)`).
  - `Replace Type Code with Class/Enum`: Use Rust enums instead of raw integer flags.

### 1.4 Long Parameter List
- **Symptoms**: A function that takes more than 3-4 parameters.
- **Causes**: Passing all required data points individually rather than grouping them into structured objects.
- **Treatment**:
  - `Introduce Parameter Object`: Combine parameters into a dedicated options/context struct.
  - `Preserve Whole Object`: Pass an existing object instead of unpacking its fields individually.

### 1.5 Data Clumps
- **Symptoms**: The same group of 3-4 variables frequently appear together in function parameters or struct fields (e.g., `x`, `y`, `z` or `start`, `end`, `step`).
- **Causes**: Poor abstraction of naturally cohesive data.
- **Treatment**:
  - `Extract Struct / Class`: Move the clumped fields into a unified data type.
  - `Introduce Parameter Object`: Group parameters when passing them to methods.

---

## 2. Object-Orientation & Architecture Abusers

These smells occur when object-oriented or domain-driven design principles are incompletely or incorrectly applied.

### 2.1 Switch / Chained Match Statements
- **Symptoms**: Identical `switch` or `if-else` / `match` blocks scattered across multiple places checking the same state or type tag.
- **Causes**: Missing polymorphism or failure to leverage dispatch mechanisms.
- **Treatment**:
  - `Replace Conditional with Polymorphism / Trait`: Use dynamic trait dispatch or standard polymorphism.
  - `Replace Type Code with State/Strategy`: Delegate state-dependent behavior to state objects or strategy closures.

### 2.2 Temporary Field
- **Symptoms**: Struct/class fields that are populated and valid only under specific circumstances, remaining `None` or uninitialized otherwise.
- **Causes**: Mixing distinct operational modes into a single unified object.
- **Treatment**:
  - `Extract Class / Struct`: Move temporary fields and the algorithms that use them into dedicated context objects.

### 2.3 Refused Bequest
- **Symptoms**: A subclass or derived struct uses only a fraction of the methods or fields inherited from its parent.
- **Causes**: Misusing inheritance for code reuse rather than subtyping (is-a relationship).
- **Treatment**:
  - `Replace Inheritance with Delegation / Composition`: Prefer composition over deep inheritance.
  - `Extract Superclass / Subclass`: Restructure hierarchy so parents contain only shared behaviors.

### 2.4 Alternative Classes with Different Interfaces
- **Symptoms**: Two classes/structs perform similar functions but have different method names or parameter layouts.
- **Causes**: Duplicate work by different developers without unifying interface contracts.
- **Treatment**:
  - `Rename Method`: Standardize method names.
  - `Extract Trait / Interface`: Define a common trait both structs implement.

---

## 3. Change Preventers

Smells that mean changing one piece of code forces you to make many small, spread-out edits elsewhere.

### 3.1 Divergent Change
- **Symptoms**: You find yourself having to change many unrelated methods inside a single class whenever you make one type of change (e.g., "whenever I change database schema, I edit 5 methods in `UserSystem`").
- **Causes**: Single class having multiple responsibilities.
- **Treatment**:
  - `Extract Class`: Separate responsibilities into distinct classes/modules.

### 3.2 Shotgun Surgery
- **Symptoms**: Making a single logical change requires making small edits to dozens of different classes/files.
- **Causes**: Responsibility is fragmented across too many modules.
- **Treatment**:
  - `Move Method / Move Field`: Gather related behavior into a single central module.
  - `Inline Class`: Merge overly fragmented classes back together.

### 3.3 Parallel Inheritance Hierarchies
- **Symptoms**: Whenever you create a subclass for class `A`, you are forced to also create a corresponding subclass for class `B`.
- **Causes**: Tight coupling across parallel class trees.
- **Treatment**:
  - `Move Method / Field`: Combine the hierarchies or substitute delegation for one tree.

---

## 4. Dispensables

Elements that are unnecessary, redundant, or dead, which clutter the codebase.

### 4.1 Duplicate Code
- **Symptoms**: The same code structure appears in more than one place.
- **Causes**: Copy-pasting code fragments instead of abstracting common behavior.
- **Treatment**:
  - `Extract Method`: Move duplicated code into a single shared function.
  - `Pull Up Method`: Move identical methods in subclasses up to a shared parent or trait implementation.

### 4.2 Comments (as a smell)
- **Symptoms**: Extensive explanatory comments explaining *what* dirty code is doing because the code itself is unreadable.
- **Causes**: Poor naming, long methods, complex conditional logic.
- **Treatment**:
  - `Extract Method`: Turn code blocks into well-named methods, making comments obsolete.
  - `Rename Method`: Give functions self-documenting names.

### 4.3 Dead Code
- **Symptoms**: Unused variables, parameters, fields, methods, or unreachable branches.
- **Causes**: Leftover code from past refactorings or obsolete features.
- **Treatment**:
  - `Remove Dead Code`: Delete unused code ruthlessly. Git history preserves past versions.

### 4.4 Speculative Generality
- **Symptoms**: Unused abstract base classes, unused generic parameters, or hooks created "just in case we need them in the future".
- **Causes**: Over-engineering and premature optimization/abstraction.
- **Treatment**:
  - `Collapse Hierarchy`: Remove unneeded abstract layers.
  - `Inline Class / Method`: Eliminate unused abstractions.

### 4.5 Lazy Class / Freeloader
- **Symptoms**: A class or struct that does very little, taking up space without adding meaningful value.
- **Causes**: Downsizing of a class after refactoring, leaving behind a shell.
- **Treatment**:
  - `Inline Class`: Merge the minor class into another object.

---

## 5. Couplers

Smells that contribute to excessive coupling between modules or excessive delegation.

### 5.1 Feature Envy
- **Symptoms**: A method accesses the data or methods of another object far more than its own.
- **Causes**: Functionality placed in the wrong struct/module.
- **Treatment**:
  - `Move Method`: Move the envious method into the class whose data it reads most.

### 5.2 Inappropriate Intimacy
- **Symptoms**: Two classes spend too much time probing into each other's private/internal fields and helper methods.
- **Causes**: Over-coupled modules violating encapsulation.
- **Treatment**:
  - `Move Method / Field`: Place functionality where data lives.
  - `Extract Class`: Move shared intimate data into a third dedicated object.

### 5.3 Message Chains (Dog-Walking)
- **Symptoms**: Code calling long chains of accessors: `a.get_b().get_c().get_d().do_something()`.
- **Causes**: Client navigation through navigation paths across object boundaries.
- **Treatment**:
  - `Hide Delegate`: Add a helper method on `a` that delegates directly (`a.do_something_on_d()`).

### 5.4 Middle Man
- **Symptoms**: A class performs only one action: delegating work to another class.
- **Causes**: Over-application of encapsulation leading to useless pass-through layers.
- **Treatment**:
  - `Remove Middle Man`: Allow callers to interact directly with the end object.
