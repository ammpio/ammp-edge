PROJECT_NAME=ammp-edge
COMPOSE_FILE=tests/docker-compose.yml
IMAGE_NAME=ammp-edge_image

.PHONY: docker-build docker-run docker-clean clean

docker-build:
	docker-compose -f ${COMPOSE_FILE} build  # --progress=plain

docker-run:
	$(MAKE) docker-build
	docker-compose -f ${COMPOSE_FILE} up -d

docker-clean:
	docker-compose -f ${COMPOSE_FILE} down
	docker rmi -f ${IMAGE_NAME}

clean:
	find . -name '*.pyc' -delete
	find . -name '__pycache__' -type d | xargs rm -fr
	rm -rf .pytest_cache
