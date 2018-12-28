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

It also watch changes recursively if a directory is provided!

```bash
echo "dir/to/watch" | entr echo 'Do something'
```

## Compilation

```bash
cargo build --release
```

## Arch Linux installation

```bash
makepkg -sif
```