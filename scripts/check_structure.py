"""Check Thor's source placement conventions without building CEF."""

from pathlib import Path
import re
import sys


BLOCK_DELIMITER = re.compile(r"/\*|\*/")
RAW_STRING_START = re.compile(r'(?:br|cr|r)(#*)"')
STRING_OR_CHARACTER = re.compile(
    r'"(?:\\.|[^"\\])*"'
    r"|'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'",
    re.DOTALL,
)


def rust_code(source):
    """Mask comments and literals before checking top-level module declarations."""
    code = []
    index = 0
    while index < len(source):
        start = index
        if source.startswith("//", index):
            end = source.find("\n", index)
            index = len(source) if end < 0 else end
        elif source.startswith("/*", index):
            index += 2
            depth = 1
            while depth and index < len(source):
                token = BLOCK_DELIMITER.search(source, index)
                if token is None:
                    index = len(source)
                    break
                depth += 1 if token[0] == "/*" else -1
                index = token.end()
        elif raw := RAW_STRING_START.match(source, index):
            closing = '"' + raw[1]
            end = source.find(closing, raw.end())
            index = len(source) if end < 0 else end + len(closing)
        elif literal := STRING_OR_CHARACTER.match(source, index):
            index = literal.end()
        else:
            code.append(source[index])
            index += 1
            continue
        code.append(re.sub(r"[^\n]", " ", source[start:index]))
    return "".join(code)


def violations(root):
    problems = []
    sources = {}
    source_roots = [root / "src", *sorted((root / "crates").glob("*/src"))]
    for source_root in source_roots:
        for path in sorted(source_root.rglob("*.rs")):
            relative = path.relative_to(root).as_posix()
            if not re.fullmatch(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)*", path.stem):
                problems.append(f"{relative}: use a snake_case Rust filename")
            directory = path.with_suffix("")
            if path.stem != "mod" and directory.is_dir():
                destination = (directory / "mod.rs").relative_to(root).as_posix()
                problems.append(f"{relative}: move module entry to {destination}")
            if path.parent == source_root and path.name in {"lib.rs", "main.rs"}:
                continue
            if path.parent == source_root / "bin":
                if path.stem.startswith("test_"):
                    problems.append(f"{relative}: place diagnostic programs in examples/")
                continue
            if path.name == "main.rs" and path.parent.parent == source_root / "bin":
                continue
            module = path.parent.name if path.name == "mod.rs" else path.stem
            owner = path.parent.parent if path.name == "mod.rs" else path.parent
            entries = ("lib.rs", "main.rs") if owner == source_root else ("mod.rs", "main.rs")
            declaration = re.compile(
                rf"^(?:pub(?:\([^)]*\))?\s+)?mod\s+{re.escape(module)}\s*;", re.MULTILINE
            )
            for entry in entries:
                source = owner / entry
                if source.is_file() and source not in sources:
                    sources[source] = rust_code(source.read_text())
            if not any(
                declaration.search(sources[owner / entry])
                for entry in entries if (owner / entry).is_file()
            ):
                problems.append(f"{relative}: no parent declares mod {module}")
    for path in sorted(root.glob("*.rs")):
        if path.name != "build.rs":
            problems.append(
                f"{path.name}: place Rust code in src/, tests/, benches/, or examples/"
            )
    return problems


if __name__ == "__main__":
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
    problems = violations(root)
    for problem in problems:
        print(problem)
    sys.exit(1 if problems else 0)
