.PHONY: build-web build-code-mode-host run run-server run-web build test lint lint-web proto-gen proto-check
.PHONY: bazel-rust bazel-ci
.PHONY: reset

CARGO_HOME ?= $(CURDIR)/.cargo
CARGO_TARGET_DIR ?= $(CURDIR)/target
RUST_HOST_TARGET ?= $(shell rustc -vV | sed -n 's/^host: //p')
CODE_MODE_HOST_PATH := $(CARGO_TARGET_DIR)/debug/codex-code-mode-host
export CARGO_HOME

build-web:
	cd web && npm run build

build-code-mode-host: $(CODE_MODE_HOST_PATH)

$(CODE_MODE_HOST_PATH): build/codex/fetch-code-mode-host.sh
	bash build/codex/fetch-code-mode-host.sh "$(RUST_HOST_TARGET)" "$@"

build: build-code-mode-host
	cargo build -p agenthub --bin agenthub
	cargo build -p agenthub-daemon --bin agenthubd

run: run-server

run-server: build-web build-code-mode-host
	cargo build -p agenthub --bin agenthub
	cargo run -p agenthub-daemon --bin agenthubd --

test:
	cargo build -p agenthub-daemon --bin agenthubd
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
