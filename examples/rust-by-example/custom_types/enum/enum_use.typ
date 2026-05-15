// Adapted from rust-by-example/custom_types/enum/enum_use.md (see ../../NOTICES.md).

#import "../../../../packages/evcxr/lib.typ" as evcxr

=== use

The `use` declaration can be used to avoid typing the full module path to access a name:

#evcxr.rust-main(id: "rbe-custom-types-enum-use", ```rust
// An attribute to hide warnings for unused code.
#![allow(dead_code)]

enum Stage {
    Beginner,
    Advanced,
}

enum Role {
    Student,
    Teacher,
}

fn main() {
    // Explicitly `use` each name so they are available without
    // manual scoping.
    use Stage::{Beginner, Advanced};
    // Automatically `use` each name inside `Role`.
    use Role::*;

    // Equivalent to `Stage::Beginner`.
    let stage = Beginner;
    // Equivalent to `Role::Student`.
    let role = Student;

    match stage {
        // Note the lack of scoping because of the explicit `use` above.
        Beginner => println!("Beginners are starting their learning journey!"),
        Advanced => println!("Advanced learners are mastering their subjects..."),
    }

    match role {
        // Note again the lack of scoping.
        Student => println!("Students are acquiring knowledge!"),
        Teacher => println!("Teachers are spreading knowledge!"),
    }
}
```)

==== See also

`match` and `use`.
