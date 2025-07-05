build:
    wasm-pack build --target web --out-dir pkg

serve:
    bunx serve .

build-and-serve:
    just build
    just serve
