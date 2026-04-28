# Fast2Flow Build System
#
# Host crates and WASM components use the root workspace.

.PHONY: build wasm pack test lint fmt check clean help

# Default target
all: build

## Build host crates
build:
	cargo build --workspace

## Build WASM components (requires wasm32-wasip2 target)
wasm:
	cargo build -p fast2flow-component-indexer -p fast2flow-component-matcher -p fast2flow-component-router --target wasm32-wasip2 --release

## Build individual WASM components
wasm-indexer:
	cargo build -p fast2flow-component-indexer --target wasm32-wasip2 --release

wasm-matcher:
	cargo build -p fast2flow-component-matcher --target wasm32-wasip2 --release

wasm-router:
	cargo build -p fast2flow-component-router --target wasm32-wasip2 --release

## Build fast2flow.gtpack
pack: wasm
	@mkdir -p dist/components
	@cp target/wasm32-wasip2/release/fast2flow_component_indexer.wasm dist/components/indexer.wasm 2>/dev/null || true
	@cp target/wasm32-wasip2/release/fast2flow_component_matcher.wasm dist/components/matcher.wasm 2>/dev/null || true
	@cp target/wasm32-wasip2/release/fast2flow_component_router.wasm dist/components/router.wasm 2>/dev/null || true
	@cp packs/fast2flow/pack.yaml dist/
	@cp -r packs/fast2flow/flows dist/
	@echo "Pack artifacts written to dist/"

## Run all tests (host crates only - WASM tests need native target)
test:
	cargo test --workspace --all-features

## Run lint checks
lint:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

## Format code
fmt:
	cargo fmt --all

## Full check (format + lint + test + build)
check:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-features
	cargo build --workspace

## Clean build artifacts
clean:
	cargo clean
	rm -rf dist/

## Bundle indexing (convenience)
bundle-index:
	@echo "Usage: make bundle-index BUNDLE=./path/to/bundle TENANT=demo TEAM=default OUTPUT=./state/indexes"
	@test -n "$(BUNDLE)" || (echo "Error: BUNDLE is required" && exit 1)
	cargo run -p greentic-fast2flow -- bundle index \
		--bundle $(BUNDLE) \
		--output $(or $(OUTPUT),./state/indexes) \
		--tenant $(or $(TENANT),demo) \
		--team $(or $(TEAM),default) \
		--generate-docs

## Show help
help:
	@echo "Fast2Flow Build Targets:"
	@echo ""
	@echo "  build         Build host crates"
	@echo "  wasm          Build all WASM components"
	@echo "  wasm-indexer  Build indexer component"
	@echo "  wasm-matcher  Build matcher component"
	@echo "  wasm-router   Build router component"
	@echo "  pack          Build gtpack artifacts to dist/"
	@echo "  test          Run all tests"
	@echo "  lint          Run format and clippy checks"
	@echo "  fmt           Format all code"
	@echo "  check         Full CI check (fmt + lint + test + build)"
	@echo "  clean         Clean all build artifacts"
	@echo "  bundle-index  Run bundle indexing (set BUNDLE=path)"
	@echo "  help          Show this help"
