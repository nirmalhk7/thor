.PHONY: dev check-structure

check-structure:
	python3 -m unittest discover -s tests -p test_repository_structure.py
	python3 scripts/check_structure.py

# CEF needs the app bundle's framework and sandboxed helper executables.
dev:
	cargo build --locked --release --features cef-bundle --bin Thor --bin thor-labs --bin thor-package
	cargo run --locked --release --features cef-bundle --bin thor-package -- dist/cef
	./dist/cef/Thor.app/Contents/MacOS/Thor --developer-mode
