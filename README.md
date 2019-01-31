# Entr

Cross platform way to run arbitrary commands when files change.

## Usage

```bash
echo "filenames" | entr command
```

Example:

```bash
touch /tmp/file.txt
find /tmp -type f -name "*.txt" | entr -p echo 'Captain! Look!'

# in another terminal...
echo "That's got to be the best pirate I've ever seen" >> /tmp/file.txt
```

For commands that uses builtins, pipes or output redirection that needs to run in a shell, there's a `-s` flag that uses the default system shell:

```bash
find /tmp -type f -name "*.txt" | entr -s "echo 'Captain! Look!' | rev"
```

It also watch changes recursively if a directory is provided!

```bash
echo "dir/to/watch" | entr -R echo 'Do something'
```

## Compilation

```bash
cargo build --release
```

## Arch Linux installation

```bash
makepkg -sif
```
