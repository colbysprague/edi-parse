SHELL := /bin/bash
.DEFAULT_GOAL := help

CARGO   ?= cargo
PYTHON  ?= python3
N       ?= 1000
T       ?=
GEN_DIR := data/generated
BIN     := ./target/release/edi-parse

.PHONY: help build release run test test-one fmt lint gen-data bench baseline clean

help: ## List all targets
	@echo "usage: make <target>"
	@echo
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

build: ## Debug build
	$(CARGO) build

release: ## Optimized build
	$(CARGO) build --release

run: ## Summarize data/samples (debug build)
	$(CARGO) run -- summary data/samples

test: ## Run every test (all files, even after failures)
	$(CARGO) test --no-fail-fast

test-one: ## Run one test file, e.g. make test-one T=tokenizer_tests
	@if [ -z "$(T)" ]; then \
		echo "usage: make test-one T=<name>   (one of: $$(ls tests/*.rs | xargs -n1 basename | sed 's/\.rs$$//' | tr '\n' ' '))"; \
		exit 1; \
	fi
	$(CARGO) test --test $(T)

fmt: ## Format code with rustfmt
	$(CARGO) fmt

lint: ## Clippy on all targets, warnings are errors
	$(CARGO) clippy --all-targets -- -D warnings

gen-data: ## Generate N synthetic files into data/generated (make gen-data N=1000)
	$(PYTHON) scripts/gen_data.py --count $(N) --out $(GEN_DIR)/

bench: ## Criterion benchmarks (run make gen-data first for process_dir)
	$(CARGO) bench

baseline: release ## Time the Python baseline vs the Rust release build on data/generated
	@if [ ! -d $(GEN_DIR) ]; then echo "no $(GEN_DIR); run: make gen-data N=$(N)"; exit 1; fi
	@echo "files: $$(ls $(GEN_DIR) | wc -l | tr -d ' ')"
	@echo; echo "== Python baseline (sequential) =="
	@time $(PYTHON) scripts/baseline.py $(GEN_DIR) > /dev/null
	@echo; echo "== Rust release, sequential =="
	@time $(BIN) summary --sequential $(GEN_DIR) > /dev/null 2>&1
	@echo; echo "== Rust release, parallel =="
	@time $(BIN) summary $(GEN_DIR) > /dev/null 2>&1

clean: ## Remove build output and generated data
	$(CARGO) clean
	rm -rf $(GEN_DIR)
