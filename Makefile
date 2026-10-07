.DEFAULT_GOAL := build
.PHONY: build test lint install uninstall package check-install-dir

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

check-install-dir:
	@test -n "$(strip $(INSTALL_DIR))" || { echo "INSTALL_DIR must not be empty" >&2; exit 1; }

install: check-install-dir build
	mkdir -p "$(INSTALL_DIR)"
	install -m 755 bin/heads "$(INSTALL_DIR)/heads"
	@if [ "$$(uname -s)" = Darwin ]; then codesign --force -s - "$(INSTALL_DIR)/heads"; fi

uninstall: check-install-dir
	rm -f -- "$(INSTALL_DIR)/heads"
