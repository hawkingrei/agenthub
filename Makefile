.PHONY: build-web run run-server run-web build test lint lint-web proto-gen proto-check
.PHONY: bazel-rust bazel-ci
.PHONY: reset

CARGO_HOME ?= $(CURDIR)/.cargo
export CARGO_HOME

build-web:
	cd web && npm run build

build:
	cargo build -p agenthub -p agenthub-codex-acp -p agenthub-acp-adapter

run: run-server

run-server: build-web
	cargo build -p agenthub-codex-acp
	cargo build -p agenthub-acp-adapter
	cargo run -p agenthub --

test:
	cargo test
	cd web && npm test

lint-web:
	cd web && npm run lint

lint: lint-web

bazel-rust:
	bazel build //...
	bazel test //...

bazel-ci:
	bazel build //...
	bazel test //...

proto-gen:
	./scripts/check_team_proto_codegen.sh --write

proto-check:
	./scripts/check_team_proto_codegen.sh --check


reset:
	rm -f $$HOME/.agenthub/agenthub.db
