.PHONY: build test test-verbose clean help all

COMPILER = ./target/debug/rc
DIST_DIR = ./dist

TESTS = \
test_simple \
test_and test_logical_ops \
test_list_indexing \
test_list_length \
test_list_ops \
test_fib \
test_while \
error_test_div_zero \
error_test_unmatched_brace \
error_test_unmatched_bracket \
error_test_invalid_list

all: build

help:
	@echo "rc (Riff Compiler) - Available targets:"
	@echo "  make build          - Build the compiler"
	@echo "  make test           - Run all tests (compiles and checks outputs if expected files exist)"
	@echo "  make test-verbose   - Run tests with detailed output (prints program output)"
	@echo "  make test-all       - Run all tests including error detection"
	@echo "  make clean          - Clean build artifacts and dist"
	@echo "  make run FILE=path  - Compile and run a specific .riff file"
	@echo "  make run-verbose FILE=path - Compile and run a specific .riff file with detailed output"
	@echo ""
	@echo "Examples:"
	@echo "  make build"
	@echo "  make test"
	@echo "  make run FILE=code/euler_one.riff"

build:
	cargo build

# Ensure dist dir exists
$(DIST_DIR):
	mkdir -p $(DIST_DIR)

# Test runner: compiles each test and compares output to expected if available
test: build | $(DIST_DIR)
	@echo "Running tests..."
	@set -e; \
	for t in $(TESTS); do \
		printf "Testing %-20s" "$$t"; \
		if ! $(COMPILER) tests/$$t.riff > /dev/null 2>&1; then \
			tmp_err=$$(mktemp); \
			$(COMPILER) tests/$$t.riff > /dev/null 2>$$tmp_err || true; \
			if [ -f tests/expected/$$t.err ]; then \
				if cmp -s $$tmp_err tests/expected/$$t.err; then \
					echo " - ✓ (expected compile error)"; \
				else \
					echo " - ✗ (compiler error differs)"; \
					printf "Expected:\n"; cat tests/expected/$$t.err; printf "\nGot:\n"; cat $$tmp_err; printf "\n"; \
				fi; \
			else \
				echo " - ✗ (compile failed)"; \
				cat $$tmp_err; \
			fi; \
			rm -f $$tmp_err; \
		fi; \
		if [ -f $(DIST_DIR)/$$t ]; then \
			out=$$(mktemp); \
			$(DIST_DIR)/$$t >$$out 2>&1 || true; \
			if [ -f tests/expected/$$t.out ]; then \
				if cmp -s $$out tests/expected/$$t.out; then \
					echo " - ✓"; \
				else \
					echo " - ✗ (output differs)"; \
					printf "Expected:\n"; cat tests/expected/$$t.out; printf "\nGot:\n"; cat $$out; printf "\n"; \
				fi; \
			else \
				echo " - (no expected file) output:"; cat $$out; \
			fi; \
			rm -f $$out; \
		else \
			echo " - missing binary"; \
		fi; \
	done; \
	echo "Test run complete. To assert outputs, create files under tests/expected/<name>.out"

test-verbose: build | $(DIST_DIR)
	@echo "=== Running tests verbosely ==="
	@for t in $(TESTS); do \
		echo "--- $$t ---"; \
		if $(COMPILER) code/$$t.riff; then \
			if [ -f $(DIST_DIR)/$$t ]; then \
				echo "Running $(DIST_DIR)/$$t..."; \
				$(DIST_DIR)/$$t || true; \
			else \
				echo "Binary missing: $(DIST_DIR)/$$t"; \
			fi; \
		else \
			echo "Compile failed for $$t"; \
		fi; \
	done

# Run a specific file
run: build
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make run FILE=path/to/file.riff"; \
		exit 1; \
	fi
	@$(COMPILER) $(FILE) > /dev/null 2>&1
	@BASENAME=$$(basename "$(FILE)" .riff); \
	if [ -f "$(DIST_DIR)/$$BASENAME" ]; then \
		$(DIST_DIR)/$$BASENAME; \
	fi

# Run a specific file verbosely
run-verbose: build
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make run-verbose FILE=path/to/file.riff"; \
		exit 1; \
	fi
	@$(COMPILER) $(FILE)
	@BASENAME=$$(basename "$(FILE)" .riff); \
	if [ -f "$(DIST_DIR)/$$BASENAME" ]; then \
		echo "Running $$BASENAME..."; \
		$(DIST_DIR)/$$BASENAME; \
	fi

clean:
	cargo clean
	rm -rf $(DIST_DIR)
	rm -f test_results.log
