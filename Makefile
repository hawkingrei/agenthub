.PHONY: run run-server run-web build test lint lint-web proto-gen proto-check
.PHONY: bazel-rust bazel-ci
.PHONY: reset

CARGO_HOME ?= $(CURDIR)/.cargo
export CARGO_HOME

WEB_BUILD_STAMP := web/dist/.build-stamp
WEB_BUILD_INPUTS := $(shell find web/src web/public -type f 2>/dev/null) \
	web/package.json \
	web/package-lock.json \
	web/tsconfig.json \
	web/vite.config.ts \
	web/index.html

build-web: $(WEB_BUILD_STAMP)

$(WEB_BUILD_STAMP): $(WEB_BUILD_INPUTS)
	cd web && npm run build
	@mkdir -p $(dir $@)
	@touch $@

build:
	cargo build -p agenthub -p agenthub-codex-acp

run: run-server

run-server: build-web
	cargo build -p agenthub-codex-acp
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
