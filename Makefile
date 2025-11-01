# =============================================================================
# Project: Port4k
# Local Dev & CI Consistency Makefile
# =============================================================================

# ----------- CONFIG ----------------------------------------------------------

RUST_DIR       ?= port4k
DOCKER_IMAGE   ?= port4k
POSTGRES_URL   ?= postgres://postgres:postgres@localhost:5432/port4k_test

CARGO          ?= cargo
NPM            ?= npm

# -----------------------------------------------------------------------------
# PHONY TARGETS
# -----------------------------------------------------------------------------
.PHONY: all fmt lint test rust docker-build docker-run clean help

all: fmt lint test ## Run all checks (fmt + lint + test)

help:
	@echo ""
	@echo "Available targets:"
	@echo "  make fmt            - Format all code (Rust, JS)"
	@echo "  make lint           - Run all linters"
	@echo "  make test           - Run all test suites"
	@echo "  make rust-test      - Run Rust tests with Postgres"
	@echo "  make docker-build   - Build Docker image"
	@echo "  make docker-run     - Run container locally"
	@echo "  make clean          - Clean build artifacts"
	@echo ""

# -----------------------------------------------------------------------------
# Formatting
# -----------------------------------------------------------------------------
fmt: rust-fmt ## Format all code (Rust, JS)

rust-fmt:
	$(CARGO) fmt --all -- --check

# -----------------------------------------------------------------------------
# Linting
# -----------------------------------------------------------------------------
lint: rust-lint ## Run all linters

rust-lint:
	cd $(RUST_DIR) && $(CARGO) clippy --all-targets --all-features -- -D warnings

# -----------------------------------------------------------------------------
# Testing
# -----------------------------------------------------------------------------
test: rust-test ## Run all tests

rust-test:
	@echo "🚀 Running Rust tests..."
	cd $(RUST_DIR) && DATABASE_URL=$(POSTGRES_URL) $(CARGO) test --all --all-features --verbose

# -----------------------------------------------------------------------------
# Docker targets
# -----------------------------------------------------------------------------
docker-build:
	docker build -t $(DOCKER_IMAGE):latest .

docker-run:
	docker run --rm -it -p 8080:8080 $(DOCKER_IMAGE):latest

# -----------------------------------------------------------------------------
# Cleanup
# -----------------------------------------------------------------------------
clean:
	@echo "🧹 Cleaning..."
	cd $(RUST_DIR) && $(CARGO) clean