SHELL := /bin/bash
.DEFAULT_GOAL := help

CARGO   ?= cargo
PYTHON  ?= python3
N       ?= 1000
T       ?=
GEN_DIR := data/generated
BIN     := ./target/release/edi-parse
RUNS    ?= 10
SCALE_NS  ?= 100 1000 10000
SCALE_DIR := data/scale
BUDGET  ?= 2
MAX_N   ?= 256000
LOAD_DIR  := data/load
EXAMPLES_DIR ?= examples/x12
EXAMPLES_OUT ?= examples/output/x12

# $(call TIME_ALL,dir): time the three implementations on dir with warm-up runs.
TIME_ALL = $(PYTHON) scripts/bench.py --runs $(RUNS) \
	--cmd python "$(PYTHON) scripts/baseline.py $(1)" \
	--cmd rust-seq "$(BIN) summary --sequential $(1)" \
	--cmd rust-par "$(BIN) summary $(1)"

.PHONY: help build release run examples test test-one fmt lint gen-data bench baseline scaling load clean

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

examples: release ## Parse every file in examples/x12 into examples/output/x12
	@if [ ! -d $(EXAMPLES_DIR) ]; then echo "no $(EXAMPLES_DIR); set EXAMPLES_DIR=<path to .edi files>"; exit 1; fi
	rm -rf $(EXAMPLES_OUT)
	$(BIN) parse $(EXAMPLES_DIR) --out $(EXAMPLES_OUT) --pretty

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

baseline: release ## Time Python vs Rust seq/parallel on data/generated (RUNS=10)
	@if [ ! -d $(GEN_DIR) ]; then echo "no $(GEN_DIR); run: make gen-data N=$(N)"; exit 1; fi
	@echo "files: $$(ls $(GEN_DIR) | wc -l | tr -d ' ')"
	@$(call TIME_ALL,$(GEN_DIR))

scaling: release ## Run baseline at N=100, 1000, 10000 (override with SCALE_NS="...")
	@for n in $(SCALE_NS); do \
		dir=$(SCALE_DIR)/$$n; \
		if [ ! -d $$dir ]; then $(PYTHON) scripts/gen_data.py --count $$n --out $$dir/ > /dev/null; fi; \
		echo; echo "== N=$$n =="; \
		$(call TIME_ALL,$$dir); \
	done

load: release ## Double batch size until each impl exceeds BUDGET secs (BUDGET=2 MAX_N=256000)
	@$(PYTHON) scripts/loadtest.py --bin $(BIN) --python $(PYTHON) --budget $(BUDGET) --max $(MAX_N)

clean: ## Remove build output and generated data
	$(CARGO) clean
	rm -rf $(GEN_DIR) $(SCALE_DIR) $(LOAD_DIR)
