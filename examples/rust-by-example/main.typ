// Adapted from rust-by-example (see NOTICES.md). Snapshot 898f0ac.

#import "../../packages/evcxr/lib.typ" as evcxr

#evcxr.setup()

#set page(numbering: "1")
#set heading(numbering: "1.")

#align(center, text(size: 24pt, weight: "bold")[
  Rust by Example
])
#align(center, text(size: 12pt)[
  Ported incrementally to Typst, evaluated through `evcxr-typst`.
])

#v(1em)

#outline(depth: 2)

#pagebreak()

#include "hello.typ"
#include "hello/comment.typ"
#include "hello/print.typ"
#include "hello/print/print_debug.typ"
#include "hello/print/print_display.typ"
#include "hello/print/print_display/testcase_list.typ"
#include "hello/print/fmt.typ"
#include "primitives.typ"
#include "primitives/literals.typ"
#include "primitives/tuples.typ"
#include "primitives/array.typ"
#include "custom_types.typ"
#include "custom_types/structs.typ"
#include "custom_types/enum.typ"
#include "custom_types/enum/enum_use.typ"
