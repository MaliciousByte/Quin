# Syntax Basics

Quin's syntax is designed to be familiar while providing modern features for safety and expressiveness.

## Variables

Variables are declared using `let`, `let mut`, or `const`.

- **Immutable**: Use `let` (cannot be changed after initialization).
- **Mutable**: Use `let mut`.
- **Constant**: Use `const` (fixed at compile-time).

```quin
let x = 10;
let mut y = 20;
y = 30; # Allowed
const PI = 3.14159;
```

## Data Types

Quin is a statically typed language that balances rigor with developer-friendly syntax.

- `int`: Signed 64-bit integer.
- `float`: 64-bit floating point.
- `str`: UTF-8 encoded string.
- `bool`: `true` or `false`.
- `any`: A type that can hold any value.
- `void`: Represents the absence of a value.

Type annotations are mandatory for some contexts and recommended everywhere else for clarity:
```quin
let count: int = 5;
let name: str = "Quin";
let is_valid: bool = true;
```

## Control Flow

### If Expressions
`if` can be used as a statement or an expression.

```quin
let sign = if x > 0 { "positive" } else { "negative" };
```

### While Loops
Standard `while` loop for iteration.

```quin
let mut i = 0;
while i < 10 {
    emit(i);
    i = i + 1;
}
```

### Match
A powerful pattern matching system.

```quin
match a {
    1 => emit("One"),
    2..10 => emit("A small number"),
    _ => emit("Something else"),
}
```
