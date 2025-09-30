PROJECT_NAME=ammp-edge
COMPOSE_FILE=tests/docker-compose.yml
IMAGE_NAME=ammp-edge_image

.PHONY: docker-build docker-run docker-clean python-clean python-dev-setup python-format python-lint python-lint-fix python-typecheck python-static-test python-build test setup-git-hooks

setup-git-hooks:
	@echo "Setting up git hooks..."
	git config core.hooksPath .githooks

docker-build:
	docker-compose -f ${COMPOSE_FILE} build  # --progress=plain

docker-run:
	docker-compose -f ${COMPOSE_FILE} up -d

docker-clean:
	docker-compose -f ${COMPOSE_FILE} down
	docker rmi -f ${IMAGE_NAME}

python-dev-setup:
	uv sync --dev
	@echo "Development environment set up. Use 'uv run' to execute commands."

python-format:
	uv run ruff format src

python-lint:
	uv run ruff check src

python-lint-fix:
	uv run ruff check --fix src

python-typecheck:
	uv run ty src

python-static-test:
	uv run ruff check src
	uv run ruff format --check src
	uv run ty src

python-build:
	uv build

python-clean:
	uv cache clean
	find . -name '*.pyc' -delete
	find . -name '__pycache__' -type d | xargs rm -fr
	find . -type d -name "*.egg-info" -exec rm -rf {} +

rust-format:
	$(MAKE) -C rust format

format:
	$(MAKE) -C rust format
	$(MAKE) python-format

test:
	$(MAKE) -C rust test
