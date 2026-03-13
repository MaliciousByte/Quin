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

Quin is a dynamically typed language. Variable types are determined at runtime, and a variable's type can change throughout its lifetime.

- `int`: Signed 64-bit integer.
- `float`: 64-bit floating point.
- `str`: UTF-8 encoded string.
- `bool`: `true` or `false`.
- `void`: Represents the absence of a value (null/nil).

While the language is dynamic, you can still use descriptive names or the `type_of()` function to inspect values:
```quin
let count = 5;
let name = "Quin";
emit(type_of(count)); # "int"
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
> [!NOTE]
> The `match` system is currently in development and supported in the parser, but full bytecode emission is pending in the compiler.

```quin
match a {
    1 => emit("One"),
    2..10 => emit("A small number"),
    _ => emit("Something else"),
}
```

## Imports & Modules

Quin contains a robust module system allowing you to pull code from the Standard Library or from other `.qn` scripts you've written.

### Standard Library Imports
```quin
use math;           # Imports the entire math module globally
use { sqrt } from math;   # Imports only the sqrt functional bind
```

### File Imports
```quin
use "paths/utils.qn";   # Dynamically evaluates and imports variables/tasks from your local file
```

Everything marked `export` in the target file is imported into your current namespace.

```quin
# In math_utils.qn
export task multiply(x, y) => x * y;

# In main.qn
use "math_utils";
emit(multiply(10, 5));
```
