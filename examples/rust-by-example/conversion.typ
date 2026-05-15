// Adapted from rust-by-example/conversion.md (see NOTICES.md).

#import "../../packages/evcxr/lib.typ" as evcxr

= Conversion

Primitive types can be converted to each other through casting.

Rust addresses conversion between custom types (i.e., `struct` and `enum`) by the use of traits. The generic conversions will use the `From` and `Into` traits. However there are more specific ones for the more common cases, in particular when converting to and from `String`s.
