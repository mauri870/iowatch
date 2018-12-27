# Entr

Cross platform way to run arbitrary commands when files change.

## Usage

```bash
echo "filenames" | entr command
```

Example:

```bash
touch /tmp/file.txt
find /tmp -type f -name "*.txt" | entr echo 'Captain! Look!'

# in another terminal...
echo "Appending to file..." >> /tmp/file.txt
```

It also watch changes recursively if a directory is provided!

```bash
echo "dir/to/watch" | entr echo 'Do something'
```

## Compilation

```bash
cargo build --release
```