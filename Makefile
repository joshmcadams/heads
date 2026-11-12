.DEFAULT_GOAL := build
.PHONY: build test lint install uninstall package check-install-dir

ifeq ($(shell uname -s),Darwin)
PLATFORM_INSTALL_DIR := $(HOME)/bin
else
PLATFORM_INSTALL_DIR := $(HOME)/.local/bin
endif

# Default to whichever of ~/.local/bin and ~/bin comes first in PATH, so the installed
# binary is the one the shell runs. Fall back to the platform's usual directory.
ifeq ($(origin INSTALL_DIR),undefined)
PATH_INSTALL_DIR := $(shell IFS=:; for d in $$PATH; do \
	case "$${d%/}" in ("$$HOME/.local/bin"|"$$HOME/bin") printf '%s' "$${d%/}"; exit;; esac; \
	done)
INSTALL_DIR := $(or $(PATH_INSTALL_DIR),$(PLATFORM_INSTALL_DIR))
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
	@found="$$(command -v heads || true)"; \
	if [ -z "$$found" ]; then \
		echo "note: $(INSTALL_DIR) is not in your PATH; add it to run heads" >&2; \
	elif [ "$$found" != "$(INSTALL_DIR)/heads" ]; then \
		echo "note: heads in your PATH is $$found, not $(INSTALL_DIR)/heads" >&2; \
	fi

uninstall: check-install-dir
	rm -f -- "$(INSTALL_DIR)/heads"
