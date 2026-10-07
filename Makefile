.PHONY: build test lint install package

ifeq ($(shell uname -s),Darwin)
INSTALL_DIR ?= $(HOME)/bin
else
INSTALL_DIR ?= $(HOME)/.local/bin
endif

build:
	@if [ "$$(uname -s)" = Linux ]; then \
		RUSTFLAGS="$${RUSTFLAGS:-} -C target-feature=+crt-static" cargo build --offline --locked --release; \
	else \
		cargo build --offline --locked --release; \
	fi
	mkdir -p bin
	cp target/release/heads bin/heads
	@if [ "$$(uname -s)" = Darwin ]; then codesign --force -s - bin/heads; fi

test:
	cargo test --offline --locked

lint:
	cargo fmt --check
	cargo clippy --offline --locked --all-targets -- -D warnings

package:
	cargo package --offline --locked

install: build
	mkdir -p "$(INSTALL_DIR)"
	cp bin/heads "$(INSTALL_DIR)/heads"
	@if [ "$$(uname -s)" = Darwin ]; then codesign --force -s - "$(INSTALL_DIR)/heads"; fi
