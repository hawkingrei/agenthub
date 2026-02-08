.PHONY: build-web run run-web build test lint lint-web
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

lint-web:
	cd web && npm run lint

lint: lint-web


reset:
	rm -f $$HOME/.agenthub/agenthub.db
