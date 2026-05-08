# Justfile for evcxr-typst.
#
# Recipes here are convenience wrappers — none of them are required to build
# the project. The most-used one is `clone-refs`, which sets up the three
# read-only sibling checkouts that the project references for path-deps and
# design-doc lookups.

# Default: list recipes.
default:
    @just --list

# Clone all three read-only reference checkouts:
#   .evcxr/                          — CGMossa/evcxr fork (path-dep target;
#                                       upstream = evcxr/evcxr; D-006, D-025)
#   .prequery/                       — typst-community/prequery (reference
#                                       Typst package for the metadata pattern)
#   .typst-wasm-minimal-protocol/    — typst-community/wasm-minimal-protocol
#                                       (reference for T-S04 plugin work)
#
# All three are gitignored. Re-running this recipe is safe; existing
# checkouts are kept untouched.
clone-refs: clone-evcxr clone-prequery clone-wasm-protocol

# Clone CGMossa/evcxr into .evcxr/ and add `upstream` pointing at evcxr/evcxr.
# Path-dep in crates/evcxr-typst/Cargo.toml resolves through this checkout.
clone-evcxr:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -e .evcxr ]; then
        echo ".evcxr/ already exists — skipping."
        exit 0
    fi
    gh repo clone CGMossa/evcxr .evcxr
    git -C .evcxr remote add upstream https://github.com/evcxr/evcxr.git
    git -C .evcxr fetch upstream

# Clone typst-community/prequery into .prequery/ (shallow, single-branch).
clone-prequery:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -e .prequery ]; then
        echo ".prequery/ already exists — skipping."
        exit 0
    fi
    gh repo clone typst-community/prequery .prequery -- --single-branch --depth=1

# Clone typst-community/wasm-minimal-protocol into .typst-wasm-minimal-protocol/.
clone-wasm-protocol:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -e .typst-wasm-minimal-protocol ]; then
        echo ".typst-wasm-minimal-protocol/ already exists — skipping."
        exit 0
    fi
    gh repo clone typst-community/wasm-minimal-protocol .typst-wasm-minimal-protocol -- --single-branch --depth=1
