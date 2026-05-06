#import "@local/evcxr:0.1.0": rust

= async / await without ceremony

evcxr detects `await` in a snippet and auto-spins-up a tokio runtime to
drive it (see `COMMON.md` "Support for async-await"). No `#[tokio::main]`,
no manual runtime construction --- you just write `async` code and `.await`
it as if you were in a function with a tokio executor already attached.

This makes Typst documents a workable surface for narrating async APIs:
the docs read like REPL transcripts, but render to PDF.

#rust(```rust
async fn double(x: u32) -> u32 {
    // Pretend this is a real I/O call --- a query, a fetch, etc.
    x * 2
}

let answer = double(21).await;
println!("the answer is {answer}");
```)

If the snippet needs more of tokio than the default minimal feature set
(timers, networking, full multi-threaded scheduler), declare an explicit
`dep` *before* the first `await` so evcxr picks up your version with the
features you want:

```typst
#dep("tokio", "1", features: ("full",))
```

Once that's in scope, snippets can use `tokio::time::sleep`,
`tokio::net::TcpStream`, channels, and the rest. The runtime stays alive
across snippets, so a future spawned in one snippet is still running
when the next snippet starts (modulo the usual caveats about persistent
`'static` ownership of the join handle).
