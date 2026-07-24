# Refactoring Techniques Catalog & Mechanics Guide

Based on [Refactoring.Guru Refactoring Techniques Catalog](https://refactoring.guru/refactoring/techniques).

This document details step-by-step mechanics and before/after patterns for primary refactoring techniques.

---

## 1. Composing Methods

Streamline methods, eliminate code duplication, and improve readability.

### 1.1 Extract Method
- **Problem**: A long function contains distinct sub-computations or logical sections.
- **Solution**: Move a cohesive code fragment into a separate, well-named function.

**Before**:
```rust
fn print_owing(player: &Player) {
    print_banner();
    
    // Calculate outstanding
    let mut outstanding = 0.0;
    for item in &player.items {
        outstanding += item.price;
    }
    
    println!("Name: {}", player.name);
    println!("Amount: {}", outstanding);
}
```

**After**:
```rust
fn print_owing(player: &Player) {
    print_banner();
    let outstanding = calculate_outstanding(player);
    print_details(player, outstanding);
}

fn calculate_outstanding(player: &Player) -> f64 {
    player.items.iter().map(|item| item.price).sum()
}

fn print_details(player: &Player, outstanding: f64) {
    println!("Name: {}", player.name);
    println!("Amount: {}", outstanding);
}
```

### 1.2 Inline Method
- **Problem**: A function body is as clear as its name, or the function is just a trivial pass-through.
- **Solution**: Replace calls to the function with its body and delete the function.

### 1.3 Replace Temp with Query
- **Problem**: You are placing the result of an expression in a temporary variable for later use.
- **Solution**: Move the expression into a pure query helper function.

### 1.4 Extract Variable
- **Problem**: You have a complex, nested expression that is hard to decipher.
- **Solution**: Assign parts of the expression to self-descriptive named local variables or constants.

---

## 2. Moving Features Between Objects & Modules

Relocate methods and fields to the places where they naturally belong.

### 2.1 Move Method
- **Problem**: A method is used more in another module/struct than in its own.
- **Solution**: Create a new method in the target struct, move code there, and convert the original method into a delegator or remove it.

### 2.2 Move Field
- **Problem**: A field is used more in another struct than in its own.
- **Solution**: Relocate the field to the target struct and update all accessors.

### 2.3 Extract Struct / Class
- **Problem**: A struct serves multiple distinct roles or holds unrelated clusters of data fields.
- **Solution**: Create a new struct and move the relevant fields and methods into it.

---

## 3. Organizing Data

Manage data representations, primitives, and access patterns.

### 3.1 Replace Primitive with Value Object (Newtype Pattern)
- **Problem**: A primitive type (`u32`, `String`) is used to represent a specific domain concept (e.g., room ID, team ID).
- **Solution**: Create a strongly-typed wrapper struct.

**Before**:
```rust
fn attach_cable(room_a: u32, room_b: u32) { ... }
```

**After**:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(pub u32);

fn attach_cable(room_a: RoomId, room_b: RoomId) { ... }
```

### 3.2 Replace Array / Tuple with Struct
- **Problem**: Positional values in arrays or tuples convey distinct semantic meanings.
- **Solution**: Convert positional fields into an explicit named struct.

---

## 4. Simplifying Conditional Expressions

Make complex branching logic clean, linear, and readable.

### 4.1 Replace Nested Conditional with Guard Clauses
- **Problem**: Deeply nested `if-else` blocks make the normal execution flow hard to follow.
- **Solution**: Use guard clauses (early returns) for special/error cases.

**Before**:
```rust
fn get_pay_amount(&self) -> f64 {
    if self.is_dead {
        dead_amount()
    } else {
        if self.is_separated {
            separated_amount()
        } else {
            if self.is_retired {
                retired_amount()
            } else {
                normal_pay_amount()
            }
        }
    }
}
```

**After**:
```rust
fn get_pay_amount(&self) -> f64 {
    if self.is_dead { return dead_amount(); }
    if self.is_separated { return separated_amount(); }
    if self.is_retired { return retired_amount(); }
    
    normal_pay_amount()
}
```

### 4.2 Replace Conditional with Match / Enum Dispatch
- **Problem**: Conditional execution branches based on an explicit state tag.
- **Solution**: Represent states using Rust `enum` variants and use pattern matching.

---

## 5. Simplifying Method Calls

Make API calls readable, robust, and self-documenting.

### 5.1 Introduce Parameter Object
- **Problem**: A group of parameters naturally go together across multiple function calls.
- **Solution**: Package them into a dedicated configuration or parameters struct.

**Before**:
```rust
fn render_viewport(&self, x: f32, y: f32, width: f32, height: f32, scale: f32, fog: bool) { ... }
```

**After**:
```rust
pub struct ViewportOpts {
    pub bounds: Rect,
    pub scale: f32,
    pub enable_fog: bool,
}

fn render_viewport(&self, opts: &ViewportOpts) { ... }
```

### 5.2 Separate Query from Modifier (Command-Query Separation)
- **Problem**: A function returns a value while simultaneously changing the state of an object.
- **Solution**: Split into two functions: one pure query function and one state-modifying command function.

---

## 6. Dealing with Generalization & Abstraction

Organize traits, hierarchies, and composition cleanly.

### 6.1 Replace Inheritance with Delegation / Composition
- **Problem**: Deep inheritance hierarchies cause rigid coupling and unwanted fields/methods.
- **Solution**: Contain an instance of the component struct and delegate specific calls to it.

### 6.2 Extract Trait / Interface
- **Problem**: Multiple structs share identical public method signatures.
- **Solution**: Define a common Trait and implement it for each struct.
