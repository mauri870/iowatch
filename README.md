# iowatch

Cross platform way to run arbitrary commands when files change.

## Usage

Download a prebuilt binary from the releases page or follow the compilation steps.

```bash
iowatch command
```

Example:

```bash
touch /tmp/file.txt
echo /tmp/file.txt | iowatch -p echo "That's got to be the best pirate I've ever seen"

# in another terminal...
echo 'Captain, Look!' >> /tmp/file.txt
```

For commands that uses builtins, pipes or output redirection that needs to run in a shell, there's a `-s` flag that uses the default system shell:

```bash
find /tmp -type f -name "/tmp/*.txt" | iowatch -s "echo 'Captain! Look!' | rev"
```

It also watch changes recursively if a directory is provided!

```bash
echo "dir/to/watch" | iowatch -R echo '!'
```

A real world use case for example is the linting of a project with hot reload:

```bash
echo "./src" | iowatch -R yarn run lint
```

Or the hot reload of a Latex project:

```bash
find . -type f \( -name "*.tex" -o -name "*.bib" -o -name "*.png" \) | iowatch tectonic paper.tex
```

## Compilation

```bash
cargo build --release
```

## Arch Linux installation

```bash
makepkg -sif
```
