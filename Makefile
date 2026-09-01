.PHONY: test lint fmt generate dev web control-plane agent

CARGO_TEST := cargo test --workspace --lib --bins --tests -- --test-threads=1

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO_TEST)
	pnpm --filter @fps/web test

generate:
	cargo run -q -p fps-control-plane -- dump-openapi > packages/api-client-generated/openapi.json
	cargo run -q -p fps-control-plane -- dump-permissions > packages/shared-web/permissions.json

control-plane:
	cargo run -p fps-control-plane -- serve

agent:
	cargo run -p fps-node-agent -- run --data-dir ./data/agent

web:
	pnpm --filter @fps/web dev

dev: generate
	@echo "Start MariaDB, then run control-plane and web in two terminals (make control-plane / make web)"
	cargo run -p fps-control-plane -- serve
