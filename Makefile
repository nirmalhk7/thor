.PHONY: dev check-structure

check-structure:
	python3 -m unittest discover -s tests -p test_repository_structure.py
	python3 scripts/check_structure.py

dev:
	./scripts/run_dev.sh --once
