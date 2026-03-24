# Vector

A JIT-compiled scripting language with a register-based VM, written in Rust.

## Features

- **Lua-like syntax** with modern ergonomics
- **Register-based bytecode VM** for efficient execution
- **JIT compilation** via Cranelift (coming soon)
- **Dynamic typing** with nil, bool, int, float, string, array, table
- **First-class functions** and closures
- **Interactive REPL** with readline support

## Quick Start

```bash
# Start REPL
vector

# Run a script
vector script.vec

# Run code directly
vector -c "print(1 + 2 * 3)"

# Show bytecode disassembly
vector --disasm script.vec
```

## Language Overview

### Variables

```
let x = 42
let mut y = "hello"
y = "world"
```

### Functions

```
fn add(a, b) {
    return a + b
}

// Anonymous functions
let double = fn(x) { return x * 2 }
```

### Control Flow

```
if condition {
    // ...
} else if other {
    // ...
} else {
    // ...
}

while condition {
    // ...
}

for i in 0..10 {
    // ...
}
```

### Collections

```
// Arrays
let nums = [1, 2, 3]
print(nums[0])
push(nums, 4)

// Tables
let person = { name: "Alice", age: 30 }
print(person["name"])
```

### Operators

- Arithmetic: `+ - * / % **`
- Comparison: `== != < <= > >=`
- Logical: `and or not`
- Bitwise: `& | ^ ~ << >>`
- String: `++` (concatenation)

## Standard Library

### Core
- `print(...)` - print values to stdout
- `type(v)` - return type name as string
- `len(v)` - length of string/array/table
- `assert(cond, msg?)` - assertion with optional message
- `error(msg)` - throw an error
- `str(v)` - convert to string

### String
- `upper(s)` - convert to uppercase
- `lower(s)` - convert to lowercase
- `trim(s)` - remove whitespace

### Math
- `abs(x)` - absolute value
- `floor(x)` - round down
- `ceil(x)` - round up
- `sqrt(x)` - square root
- `min(...)` - minimum of values
- `max(...)` - maximum of values

### Array
- `push(arr, v)` - append value
- `pop(arr)` - remove and return last

## Building

```bash
cargo build --release
```

## Examples

```
// Countdown
let mut x = 10
while x > 0 {
    print(x)
    x = x - 1
}
print("Blast off!")

// Fibonacci
fn fib(n) {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}
print(fib(20))

// Array operations
let nums = [1, 2, 3, 4, 5]
let mut sum = 0
let mut i = 0
while i < len(nums) {
    sum = sum + nums[i]
    i = i + 1
}
print("Sum:", sum)
```

## License

MIT
