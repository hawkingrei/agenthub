.PHONY: build-web run run-web build test
.PHONY: reset

CARGO_HOME ?= $(CURDIR)/.cargo
export CARGO_HOME

build-web:
	cd web && npm run build

build:
	cargo build

run: build-web
	cargo run

test:
	cargo test
	cd web && npm test


reset:
	rm -f $$HOME/.agenthub/agenthub.db
