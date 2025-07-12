#!/usr/bin/env bats

# Use the local binary instead of system-installed one
IOWATCH="./target/release/iowatch"

setup() {
  TMPDIR=$(mktemp -d)
  TESTFILE="$TMPDIR/test.txt"
  echo "initial" > "$TESTFILE"
}

teardown() {
  rm -rf "$TMPDIR"
}

@test "prints help with -h" {
  run "$IOWATCH" -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: iowatch"* ]]
}

@test "prints version with -V" {
  run "$IOWATCH" -V
  [ "$status" -eq 0 ]
  [[ "$output" =~ ^iowatch\ [0-9]+\.[0-9]+\.[0-9]+$ ]]
}

@test "exits after first run with -z" {
    run "$IOWATCH" -z -f "$TESTFILE" echo "once"
    [ "$status" -eq 0 ]
    [[ "$output" == "once" ]]
}

@test "postpones first execution with -p" {
  run timeout 1 "$IOWATCH" -p "$TESTFILE" -- echo "should not print"
  [[ "$output" != *"should not print"* ]]
}

@test "evaluates with shell using -s" {
    run timeout 1 "$IOWATCH" -f "$TESTFILE" -z -s 'echo $((2+1))'
    [[ "$output" == *"3"* ]]
}

@test "runs utility when file changes" {
  OUTFILE="$TMPDIR/iowatch.txt"

  # Start iowatch in the background, postponed, watching TESTFILE
  "$IOWATCH" -p -f "$TESTFILE" -z echo changed > "$OUTFILE" &
  WATCH_PID=$!

  # modify test file
  sleep 0.2
  echo "new line" >> "$TESTFILE"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "changed" ]]
}

@test "clears screen with -c" {
    run timeout 2 "$IOWATCH" -c -f "$TESTFILE" echo "cleared"
    [[ "$output" == *"cleared"* ]]
}


@test "runs utility after timeout with -t" {
    run timeout 2 "$IOWATCH" -t 1 -f "$TESTFILE" echo "timeout"
    [[ "$output" == *"timeout"* ]]
}

@test "uses given kill signal with -k" {
    run timeout 2 "$IOWATCH" -k SIGTERM -d 50 -f "$TESTFILE" sleep 2
    [ "$status" -eq 0 ] || [ "$status" -eq 124 ]
}

@test "reads files from stdin" {
  OUTFILE="$TMPDIR/iowatch.txt"

  # Start iowatch in the background, postponed, reading file from stdin
  bash -c "echo \"$TESTFILE\" | \"$IOWATCH\" -p -z echo changed" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2
  
  # Modify the test file
  echo "new content" >> "$TESTFILE"
  sleep 0.5

  # Kill iowatch to clean up (in case it's still running)
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that the output file was created and contains the expected message
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "changed" ]]
}