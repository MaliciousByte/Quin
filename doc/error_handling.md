# Error Handling

Quin provides structured error handling to manage runtime exceptions gracefully.

## Attempt / Rescue / Finally

The primary mechanism for error handling is the `attempt` block.

```quin
attempt {
    # Code that might fail
    let result: any = risky_operation();
    emit("Success: {result}");
} rescue (err: str) {
    # Code to handle the error
    emit("An error occurred: {err}");
} finally {
    # Code that always runs
    emit("Cleanup complete.");
}
```

- **`attempt`**: The block of code to monitor for errors.
- **`rescue`**: Captures the error value (usually a string or an object) if one is raised.
- **`finally`**: An optional block that executes regardless of whether an error was raised or caught.

## Raising Errors

You can manually trigger an error using the `raise` keyword.

```quin
task divide(a: float, b: float) -> float {
    if b == 0.0 {
        raise "Division by zero!";
    }
    return a / b;
}
```

## Error Propagation

If an error is not caught within a task, it propagates up the call stack to the caller. If it reaches the top level without being caught, the VM terminates with an error message.
