# 📚 Quin Standard Library Reference

Quin ships with a built-in standard library. Certain core functions (`emit`, `len`, `type_of`, `assert`) are available globally — no `use` statement needed. All other functions must be explicitly imported from their respective modules using `use module;` or `use { item } from module;`.

---

## 🔢 Math

| Function | Signature | Description |
|----------|-----------|-------------|
| `sqrt(x)` | `float → float` | Square root |
| `pow(base, exp)` | `float, float → float` | Exponentiation |
| `abs(x)` | `number → number` | Absolute value |
| `floor(x)` | `float → int` | Round down |
| `ceil(x)` | `float → int` | Round up |
| `round(x)` | `float → int` | Round to nearest |
| `min(a, b)` | `number, number → number` | Minimum of two values |
| `max(a, b)` | `number, number → number` | Maximum of two values |
| `PI` | `float` | π (3.14159...) |
| `E` | `float` | Euler's number (2.71828...) |

```quin
use math;

emit(math.sqrt(16));       # 4.0
emit(math.pow(2, 10));     # 1024.0
emit(math.abs(-42));       # 42
emit(math.floor(3.7));     # 3

# Or use selective imports:
use { ceil } from math;
emit(ceil(3.2));      # 4
```

---

## 🔤 String

| Function | Signature | Description |
|----------|-----------|-------------|
| `len(s)` | `str → int` | Length (also works on arrays, dicts, sets, tuples) |
| `upper(s)` | `str → str` | Uppercase |
| `lower(s)` | `str → str` | Lowercase |
| `trim(s)` | `str → str` | Strip whitespace |
| `contains(s, sub)` | `str, str → bool` | Check substring |
| `replace(s, from, to)` | `str, str, str → str` | Replace all occurrences |
| `split(s, delim)` | `str, str → str[]` | Split into array |
| `starts_with(s, pre)` | `str, str → bool` | Prefix check |
| `ends_with(s, suf)` | `str, str → bool` | Suffix check |
| `to_str(v)` | `any → str` | Convert to string |
| `to_int(v)` | `str\|float → int` | Parse/convert to int |
| `to_float(v)` | `str\|int → float` | Parse/convert to float |

```quin
use string;

let words = string.split("hello world", " ");  # ["hello", "world"]
emit(string.upper("quin"));                     # "QUIN"
emit(string.contains("hello", "ell"));          # true
```

---

## 📦 Array

| Function | Signature | Description |
|----------|-----------|-------------|
| `push(arr, val)` | `array, any → void` | Append element (mutates) |
| `pop(arr)` | `array → any` | Remove & return last element |
| `slice(arr, start, end?)` | `array, int, int? → array` | Sub-array |
| `reverse(arr)` | `array → void` | Reverse in place |
| `sort(arr)` | `array → void` | Sort in place (numbers) |
| `range(end)` | `int → int[]` | `[0, 1, ..., end-1]` |
| `range(start, end)` | `int, int → int[]` | `[start, ..., end-1]` |
| `range(start, end, step)` | `int, int, int → int[]` | With step |
| `join(arr, sep)` | `array, str → str` | Join elements |
| `map(arr, task)` | `array, task → array` | Transform elements |
| `filter(arr, task)` | `array, task → array` | Filter elements |

```quin
use array;

let nums = array.range(1, 6);  # [1, 2, 3, 4, 5]
let squares = array.map(nums, task(x) => x * x);
let evens = array.filter(nums, task(x) => x > 2);
array.push(nums, 6);
array.sort(nums);
emit(array.join(nums, ", "));  # "1, 2, 3, 4, 5, 6"
```

---

## 📥 IO

| Function | Signature | Description |
|----------|-----------|-------------|
| `emit(val)` | `any → void` | Print to stdout |
| `input(prompt?)` | `str? → str` | Read line from stdin |
| `read_file(path)` | `str → str` | Read file contents |
| `write_file(path, data)` | `str, str → void` | Write file |
| `type_of(val)` | `any → str` | Runtime type name |
| `assert(cond, msg?)` | `bool, str? → void` | Fail if condition is falsey |

```quin
use io;

let name = io.input("What's your name? ");
assert(len(name) > 0, "Name cannot be empty");
emit("Hello, " + name + "!");
emit(type_of(42));    # "int"
emit(type_of(3.14));  # "float"
```

---

## 🖥️ OS

| Function | Signature | Description |
|----------|-----------|-------------|
| `clock()` | `→ float` | Unix timestamp (seconds) |
| `exit(code?)` | `int? → never` | Exit process |
| `env(name)` | `str → str\|void` | Read environment variable |
| `args()` | `→ str[]` | Command-line arguments |

```quin
use os;

let start = os.clock();
# ... do work ...
let elapsed = os.clock() - start;
emit("Took " + elapsed + " seconds");
```
