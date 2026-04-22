.PHONY: build install dev test clean codicil

build:
	cargo build --release

install: build
	cargo install --path . --quiet

codicil:
	cd ../codicil/codicil-cli && cargo install --path . --quiet 2>/dev/null || cd ../codicil/codicil-cli && cargo install --path .

dev: install codicil
	./target/release/brief-compiler rbv $(file)

test:
	cargo test --lib

clean:
	cargo clean