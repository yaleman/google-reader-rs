.DEFAULT: help
.PHONY: help
help:
	@grep -E -h '\s##\s' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'


.PHONY: doc
doc: ## Build the docs
doc:
	cargo doc --no-deps --all-features

.PHONY: doc/open
doc/open: ## Build the docs and open them
doc/open:
	cargo doc --no-deps --all-features --open

