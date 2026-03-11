# Object-Oriented Programming (OOP)

Quin features a robust object-oriented system with classes, interfaces, and shared members.

## Classes and Inheritance

Classes are defined with the `class` keyword. Inheritance is done using `extends`.

```quin
class Animal {
    let name;
    init(name) {
        self.name = name;
    }
    
    task make_noise();
}

class Dog extends Animal {
    task make_noise() {
        emit("{self.name} says: Woof!");
    }
}
```

## Traits (Interfaces)

Traits define a contract that classes can implement using the `with` keyword.

```quin
trait Sound {
    task make_noise();
}

class Cat with Sound {
    task make_noise() {
        emit("Meow!");
    }
}
```

## Constructors (`init`)

The `init` keyword is used to define the initialization logic for a class.

## Access Modifiers

Quin supports access control for class members:
- `pub`: Public (accessible from anywhere).
- `priv`: Private (accessible only within the class).

```quin
class User {
    priv id;
    pub username;
    
    init(id, name) {
        self.id = id;
        self.username = name;
    }
}
```

## Shared Members

Replace static members with the `shared` keyword. Shared members belong to the class itself, not instances.

```quin
class MathUtils {
    shared const PI = 3.14159;
    
    shared task circle_area(r) => self.PI * r * r;
}
```
