# Collections

Quin provides several built-in collection types to handle data structures efficiently.

## Arrays
Ordered, dynamically-sized collections of elements.

```quin
let list: int[] = [1, 2, 3, 4];
emit(list[0]); # 1
```

## Dictionaries (Dicts)
Key-value pairs for fast lookup.

```quin
let user: dict<str, any> = {"name": "Alice", "age": 30};
emit(user["name"]); # Alice
```

## Sets
Collections of unique elements.

```quin
let unique_nums: set<int> = set{1, 2, 2, 3}; # Results in {1, 2, 3}
```

## Tuples
Fixed-size, heterogeneous collections.

```quin
let point: tuple<int, int, str> = (10, 20, "label");
emit(point[2]); # "label"
```

## Destructuring
> [!IMPORTANT]
> Destructuring is currently supported in the parser, but compiler implementation is pending.

```quin
let (x, y) = (100, 200);
let [first, second, ...rest] = [1, 2, 3, 4, 5];
```
