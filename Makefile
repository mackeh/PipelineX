.PHONY: help build test install clean analyze optimize docker lint fmt check all

# Configuration
CARGO := cargo
DOCKER := docker
PIPELINEX := ./target/release/pipelinex
WORKFLOW := .github/workflows/ci.yml

# Colors
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[1;33m
NC := \033[0m # No Color

help: ## Show this help message
	@echo "$(BLUE)PipelineX - CI/CD Pipeline Optimizer$(NC)"
	@echo ""
	@echo "$(GREEN)Available targets:$(NC)"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(BLUE)%-15s$(NC) %s\n", $$1, $$2}'

all: fmt lint test build ## Run all checks and build

build: ## Build release binary
	@echo "$(BLUE)Building PipelineX...$(NC)"
	$(CARGO) build --release
	@echo "$(GREEN)✓ Build complete: $(PIPELINEX)$(NC)"

build-debug: ## Build debug binary
	@echo "$(BLUE)Building debug binary...$(NC)"
	$(CARGO) build

test: ## Run all tests
	@echo "$(BLUE)Running tests...$(NC)"
	$(CARGO) test --all
	@echo "$(GREEN)✓ All tests passed$(NC)"

test-verbose: ## Run tests with output
	@echo "$(BLUE)Running tests (verbose)...$(NC)"
	$(CARGO) test --all -- --nocapture

test-integration: ## Run integration tests only
	@echo "$(BLUE)Running integration tests...$(NC)"
	$(CARGO) test --test integration_tests

lint: ## Run clippy linter
	@echo "$(BLUE)Running clippy...$(NC)"
	$(CARGO) clippy --all-targets -- -D warnings
	@echo "$(GREEN)✓ No linting issues$(NC)"

fmt: ## Format code
	@echo "$(BLUE)Formatting code...$(NC)"
	$(CARGO) fmt --all
	@echo "$(GREEN)✓ Code formatted$(NC)"

check: ## Run cargo check
	@echo "$(BLUE)Checking code...$(NC)"
	$(CARGO) check --all-targets
	@echo "$(GREEN)✓ Check complete$(NC)"

install: build ## Install binary locally
	@echo "$(BLUE)Installing PipelineX...$(NC)"
	$(CARGO) install --path crates/pipelinex-cli --force
	@echo "$(GREEN)✓ Installed: $$(which pipelinex)$(NC)"

clean: ## Clean build artifacts
	@echo "$(BLUE)Cleaning...$(NC)"
	$(CARGO) clean
	rm -f pipelinex-*.sarif pipelinex-*.json pipelinex-*.html
	@echo "$(GREEN)✓ Clean complete$(NC)"

# PipelineX commands
analyze: build ## Analyze CI pipeline
	@echo "$(BLUE)Analyzing $(WORKFLOW)...$(NC)"
	@$(PIPELINEX) analyze $(WORKFLOW)

analyze-json: build ## Analyze and output JSON
	@$(PIPELINEX) analyze $(WORKFLOW) --format json

analyze-html: build ## Generate HTML report
	@echo "$(BLUE)Generating HTML report...$(NC)"
	$(PIPELINEX) analyze $(WORKFLOW) --format html > pipelinex-report.html
	@echo "$(GREEN)✓ Report saved: pipelinex-report.html$(NC)"
	@command -v xdg-open >/dev/null 2>&1 && xdg-open pipelinex-report.html || \
	 command -v open >/dev/null 2>&1 && open pipelinex-report.html || \
	 echo "$(YELLOW)Open pipelinex-report.html in your browser$(NC)"

analyze-sarif: build ## Generate SARIF output
	@echo "$(BLUE)Generating SARIF...$(NC)"
	$(PIPELINEX) analyze $(WORKFLOW) --format sarif > pipelinex.sarif
	@echo "$(GREEN)✓ SARIF saved: pipelinex.sarif$(NC)"

optimize: build ## Show optimization suggestions
	@echo "$(BLUE)Optimization suggestions for $(WORKFLOW):$(NC)"
	@$(PIPELINEX) diff $(WORKFLOW)

optimize-generate: build ## Generate optimized pipeline
	@echo "$(BLUE)Generating optimized pipeline...$(NC)"
	$(PIPELINEX) optimize $(WORKFLOW) -o ci-optimized.yml
	@echo "$(GREEN)✓ Optimized pipeline saved: ci-optimized.yml$(NC)"

cost: build ## Calculate cost analysis
	@echo "$(BLUE)Cost analysis:$(NC)"
	@$(PIPELINEX) cost .github/workflows/

simulate: build ## Run Monte Carlo simulation
	@echo "$(BLUE)Running simulation...$(NC)"
	@$(PIPELINEX) simulate $(WORKFLOW)

graph: build ## Visualize pipeline graph
	@echo "$(BLUE)Generating graph...$(NC)"
	@$(PIPELINEX) graph $(WORKFLOW)

# Docker commands
docker-build: ## Build Docker image
	@echo "$(BLUE)Building Docker image...$(NC)"
	$(DOCKER) build -t pipelinex:local .
	@echo "$(GREEN)✓ Docker image built: pipelinex:local$(NC)"

docker-run: ## Run Docker container
	@echo "$(BLUE)Running PipelineX in Docker...$(NC)"
	$(DOCKER) run --rm -v $(PWD):/workspace:ro pipelinex:local analyze /workspace/$(WORKFLOW)

docker-shell: ## Open shell in Docker container
	@$(DOCKER) run --rm -it -v $(PWD):/workspace pipelinex:local bash

docker-compose-up: ## Start docker-compose services
	$(DOCKER)-compose up

docker-compose-analyze: ## Run analysis via docker-compose
	$(DOCKER)-compose run pipelinex analyze $(WORKFLOW)

# CI/CD Integration
pre-commit-install: ## Install pre-commit hooks
	@echo "$(BLUE)Installing pre-commit hooks...$(NC)"
	@if command -v pre-commit >/dev/null 2>&1; then \
		pre-commit install; \
		echo "$(GREEN)✓ Pre-commit hooks installed$(NC)"; \
	else \
		echo "$(YELLOW)pre-commit not found. Install with: pip install pre-commit$(NC)"; \
	fi

pre-commit-run: ## Run pre-commit on all files
	@echo "$(BLUE)Running pre-commit...$(NC)"
	pre-commit run --all-files

ci-local: all analyze ## Run full CI pipeline locally
	@echo "$(GREEN)✓ Local CI complete!$(NC)"

# Development
dev-setup: ## Setup development environment
	@echo "$(BLUE)Setting up development environment...$(NC)"
	@rustup component add rustfmt clippy
	@command -v pre-commit >/dev/null 2>&1 && pre-commit install || true
	@echo "$(GREEN)✓ Development environment ready$(NC)"

watch: ## Watch and rebuild on changes
	@echo "$(BLUE)Watching for changes...$(NC)"
	$(CARGO) watch -x "build --release" -x test

docs: ## Generate and open documentation
	@echo "$(BLUE)Generating documentation...$(NC)"
	$(CARGO) doc --no-deps --open

bench: ## Run benchmarks
	@echo "$(BLUE)Running benchmarks...$(NC)"
	$(CARGO) bench

# Release
release-dry-run: ## Dry run release build
	@echo "$(BLUE)Release dry run...$(NC)"
	$(CARGO) build --release --all-targets

release: test lint ## Build release for all platforms
	@echo "$(BLUE)Building release binaries...$(NC)"
	@echo "$(YELLOW)This requires cross-compilation tools$(NC)"
	$(CARGO) build --release --target x86_64-unknown-linux-gnu
	$(CARGO) build --release --target aarch64-apple-darwin
	$(CARGO) build --release --target x86_64-pc-windows-msvc

# Utilities
loc: ## Count lines of code
	@echo "$(BLUE)Lines of code:$(NC)"
	@tokei .

deps: ## Show dependency tree
	$(CARGO) tree

audit: ## Run security audit
	@echo "$(BLUE)Running security audit...$(NC)"
	$(CARGO) audit

update: ## Update dependencies
	@echo "$(BLUE)Updating dependencies...$(NC)"
	$(CARGO) update
