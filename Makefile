PROJECT_NAME=ammp-edge

include .env

.PHONY: docker-build docker-run docker-clean clean run

docker-build:
	docker-compose -f docker-compose.yml build

docker-run:
	$(MAKE) docker-build
	docker-compose -f docker-compose.yml up -d

docker-clean:
	docker-compose -f docker-compose.yml down
	docker rmi -f ${IMAGE_NAME}

clean:
	find . -name '*.pyc' -delete
	find . -name '__pycache__' -type d | xargs rm -fr
	rm -rf .pytest_cache

local-prepare:
	mkdir -p .local
	cp -a provisioning .local/
	ln -sf ../drivers .local/
	ln -sf ../resources .local/
	mkdir -p .local/data
	mkdir -p .local/usr
	mkdir -p .local/usr/bin
	ln -sf `which nmap` .local/usr/bin/

local-run:
	$(MAKE) docker-run
	set -a && . ./.env
	SNAP=.local \
	SNAP_COMMON=.local/data \
	pipenv run ammp_edge
