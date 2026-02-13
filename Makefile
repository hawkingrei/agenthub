.PHONY: build-web run run-web build test lint lint-web
.PHONY: bazel-rust bazel-web bazel-ci
.PHONY: reset

CARGO_HOME ?= $(CURDIR)/.cargo
export CARGO_HOME

build-web:
	cd web && npm run build

build:
	cargo build

run: build-web
	cargo build --workspace
	cargo run

test:
	cargo test
	cd web && npm test

lint-web:
	cd web && npm run lint

lint: lint-web

bazel-rust:
	bazel run //:rust_checks

bazel-web:
	bazel run //:web_checks

bazel-ci:
	bazel run //:ci_checks


reset:
	rm -f $$HOME/.agenthub/agenthub.db
