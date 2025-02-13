PROJECT_NAME=ammp-edge
COMPOSE_FILE=tests/docker-compose.yml
IMAGE_NAME=ammp-edge_image

.PHONY: docker-build docker-run docker-clean python-clean python-dev-setup python-format python-lint test

docker-build:
	docker-compose -f ${COMPOSE_FILE} build  # --progress=plain

docker-run:
	docker-compose -f ${COMPOSE_FILE} up -d

docker-clean:
	docker-compose -f ${COMPOSE_FILE} down
	docker rmi -f ${IMAGE_NAME}

python-dev-setup:
	python -m venv venv
	. venv/bin/activate && pip install -r requirements-dev.txt
	cd src && . ../venv/bin/activate && pip install -e .
	@echo "Please run '. venv/bin/activate' to enter the virtual environment"

python-format:
	isort src
	black src

python-static-test:
	isort --check src
	black --check src
	flake8 src

test:
	$(MAKE) python-static-test
	$(MAKE) -C rust test

python-clean:
	rm -rf venv/
	find . -name '*.pyc' -delete
	find . -name '__pycache__' -type d | xargs rm -fr
	find . -type d -name "*.egg-info" -exec rm -rf {} +
