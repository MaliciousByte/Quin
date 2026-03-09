# Functional Programming

Quin embraces functional programming concepts, making it easy to write clean, composable code.

## Tasks (Functions)

In Quin, functions are called `tasks`. They are first-class citizens, meaning they can be passed as arguments, returned from other tasks, and assigned to variables.

```quin
task square(x: int) -> int {
    return x * x;
}

# Arrow shorthand for simple tasks
task add(a: int, b: int) -> int => a + b;
```

## Closures & Lambdas

Quin supports anonymous tasks and full lexical scoping with variable capture.

```quin
let multiplier: any = task(factor: int) -> any => task(x: int) -> int => x * factor;
let double: any = multiplier(2);
emit(double(5)); # 10
```

## Pipe Operator (`|>`)

The pipe operator allows you to chain task calls in a readable way, passing the result of the left expression as the first argument to the right task.

```quin
let result = 5 
    |> square 
    |> double; 
# result is (5 * 5) * 2 = 50
```

## Higher-Order Tasks

Since tasks are first-class, you can easily build powerful abstractions:

```quin
task apply_twice(f, x) => f(f(x));
emit(apply_twice(square, 3)); # 81
```
