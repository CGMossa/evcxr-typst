// T-I04 smoke test: MIME passthrough — inline PNG plot + CBOR data roundtrip.
//
// Run (from repo root):
//   cargo run -p evcxr-typst -- run --allow-eval --root . examples/image/main.typ
//
// Then check:
//   examples/image/.evcxr-typst-cache/<id>.png  (valid PNG)
//   examples/image/.evcxr-typst-cache/<id>.cbor (non-empty CBOR)
//   examples/image/main.pdf                      (rendered PDF)

#import "../../packages/evcxr/lib.typ" as evcxr

= T-I04 smoke

== Dependencies

#evcxr.dep("evcxr_runtime", version: "1.1", features: ("bytes",))
#evcxr.dep("image", version: "0.24")
#evcxr.dep("ciborium", version: "0.2")

== Inline PNG plot

Generate a 64×64 gradient image and embed it:

#evcxr.rust-display(id: "img-plot", ```rust
use std::io::Cursor;
{
    let img: image::RgbImage = image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 128u8])
    });
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    evcxr_runtime::mime_type("image/png").bytes(buf.get_ref());
}
```)

== CBOR data roundtrip

Serialize a small dict via CBOR and parse it back in Typst.

The snippet emits data (evaluated by the CLI):

#evcxr.rust-data(id: "cbor-stats", ```rust
use ciborium::value::Value;
{
    let val = Value::Map(vec![
        (Value::Text("mean".into()), Value::Float(42.0)),
        (Value::Text("n".into()), Value::Integer(7.into())),
    ]);
    let mut buf: Vec<u8> = Vec::new();
    ciborium::ser::into_writer(&val, &mut buf).unwrap();
    evcxr_runtime::mime_type("application/cbor").bytes(&buf);
}
```)

Read the sidecar value back as a Typst dict:

#let stats = evcxr.rust-data-read(id: "cbor-stats", fallback: ("mean": 0.0, "n": 0))

Mean = #stats.at("mean"), n = #stats.at("n").
