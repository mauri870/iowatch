# iowatch

Cross platform way to run arbitrary commands when files change.

## Installation

Download one of the prebuilt binaries from the relases page or install it with cargo:

```bash
cargo install --git https://github.com/mauri870/iowatch
```

## Usage

Download a prebuilt binary from the releases page or follow the compilation steps.

```bash
iowatch command
```

Example:

```bash
touch /tmp/file.txt
echo /tmp/file.txt | iowatch -p echo "> file changed!"

# in another terminal...
echo 'that is a new line' >> /tmp/file.txt
```

> Note: iowatch has builtin support for .[git]ignore files ;)

For commands that uses builtins, pipes or output redirection that needs to run in a shell, there's a `-s` flag that uses the default system shell:

```bash
find /tmp -type f -name "/tmp/*.txt" | iowatch -s "echo Hello | rev"
```

It also watch changes recursively if a directory is provided!

```bash
echo "dir/to/watch" | iowatch -R echo '!'
```

A real world use case for example is the linting of a project with hot reload:

```bash
echo "./src" | iowatch -R yarn run lint
```

Or hot reload of a Go project:

```bash
echo $PWD | iowatch -R go run .
```

Or auto compile a Latex project whenever an important file changes:

```bash
find . -type f \( -name "*.tex" -o -name "*.bib" -o -name "*.png" \) | iowatch tectonic paper.tex
```

## Compilation

```bash
cargo build --release
```

For arch linux users:

```bash
makepkg -sif
```
