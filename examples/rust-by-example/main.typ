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
#include "custom_types/enum/c_like.typ"
#include "custom_types/enum/testcase_linked_list.typ"
#include "custom_types/constants.typ"
#include "variable_bindings.typ"
#include "variable_bindings/mut.typ"
#include "variable_bindings/scope.typ"
#include "variable_bindings/declare.typ"
#include "variable_bindings/freeze.typ"
#include "types.typ"
#include "types/cast.typ"
#include "types/literals.typ"
#include "types/inference.typ"
#include "types/alias.typ"
#include "conversion.typ"
#include "conversion/from_into.typ"
#include "conversion/try_from_try_into.typ"
#include "conversion/string.typ"
#include "expression.typ"
#include "flow_control.typ"
#include "flow_control/if_else.typ"
#include "flow_control/while.typ"
#include "flow_control/loop.typ"
#include "flow_control/loop/nested.typ"
#include "flow_control/loop/return.typ"
#include "flow_control/for.typ"
#include "flow_control/match.typ"
#include "flow_control/if_let.typ"
#include "flow_control/while_let.typ"
