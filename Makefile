.PHONY: build-web run run-web build
.PHONY: reset

CARGO_HOME ?= $(CURDIR)/.cargo
export CARGO_HOME

build-web:
	cd web && npm run build

build:
	mkdir -p $(CARGO_HOME)
	cargo build

run:
	cd web && npm run build
	mkdir -p $(CARGO_HOME)
	cargo run

reset:
	rm -f $$HOME/.agenthub/agenthub.db
