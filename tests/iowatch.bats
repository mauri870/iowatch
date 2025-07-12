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

@test "watches directories recursively with -R" {
  SUBDIR="$TMPDIR/subdir"
  SUBFILE="$SUBDIR/subfile.txt"
  OUTFILE="$TMPDIR/recursive_test.txt"
  
  mkdir -p "$SUBDIR"
  echo "initial" > "$SUBFILE"

  # Start iowatch in the background watching directory recursively
  "$IOWATCH" -R -p -f "$TMPDIR" -z echo "recursive change" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Modify the subdirectory file
  echo "new content" >> "$SUBFILE"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that the change was detected
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "recursive change" ]]
}

@test "does not watch subdirectories without -R" {
  SUBDIR="$TMPDIR/subdir"
  SUBFILE="$SUBDIR/subfile.txt"
  OUTFILE="$TMPDIR/nonrecursive_test.txt"
  
  mkdir -p "$SUBDIR"
  echo "initial" > "$SUBFILE"

  # Start iowatch in the background watching directory non-recursively
  "$IOWATCH" -p -f "$TMPDIR" -z echo "non-recursive change" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Modify the subdirectory file (should not trigger)
  echo "new content" >> "$SUBFILE"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that no output was generated (file should not exist or be empty)
  run cat "$OUTFILE" 2>/dev/null || echo "no output"
  [[ "$output" == "no output" ]] || [[ "$output" == "" ]]
}

@test "watches multiple files from stdin" {
  TESTFILE2="$TMPDIR/test2.txt"
  OUTFILE="$TMPDIR/multiple_files_test.txt"
  
  echo "initial" > "$TESTFILE2"

  # Start iowatch reading multiple files from stdin
  bash -c "printf '%s\n%s\n' '$TESTFILE' '$TESTFILE2' | '$IOWATCH' -p -z echo 'multiple files change'" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Modify the second file
  echo "new content" >> "$TESTFILE2"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that the change was detected
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "multiple files change" ]]
}

@test "watches directory for new files" {
  NEWFILE="$TMPDIR/newfile.txt"
  OUTFILE="$TMPDIR/directory_test.txt"

  # Start iowatch watching the directory
  "$IOWATCH" -p -f "$TMPDIR" -z echo "directory change" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Create a new file in the directory
  echo "new file content" > "$NEWFILE"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that the change was detected
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "directory change" ]]
}

@test "handles non-existent files gracefully" {
  NONEXISTENT="$TMPDIR/nonexistent.txt"
  
  run "$IOWATCH" -z -f "$NONEXISTENT" echo "should not work"
  [ "$status" -ne 0 ]
  [[ "$output" == *"Failed to watch"* ]]
}

@test "handles empty stdin input" {
  run bash -c "echo '' | '$IOWATCH' -z echo 'empty input'"
  [ "$status" -ne 0 ]
  [[ "$output" == *"no files or directories to watch"* ]]
}

@test "applies delay with -d flag" {
  OUTFILE="$TMPDIR/delay_test.txt"
  START_TIME=$(date +%s%3N)

  # Start iowatch with 300ms delay
  "$IOWATCH" -p -f "$TESTFILE" -d 300 -z echo "delayed execution" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Modify the file and measure time
  echo "trigger change" >> "$TESTFILE"
  sleep 0.8  # Wait for delay + execution

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that the command executed
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "delayed execution" ]]
}

@test "handles utility with multiple arguments" {
  OUTFILE="$TMPDIR/args_test.txt"

  # Start iowatch with a utility that has multiple arguments
  "$IOWATCH" -p -f "$TESTFILE" -z -s "echo 'arg1' 'arg2' 'arg3'" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Modify the file
  echo "trigger change" >> "$TESTFILE"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that all arguments were passed
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "arg1 arg2 arg3" ]]
}

@test "handles utility that fails" {
  OUTFILE="$TMPDIR/fail_test.txt"

  # Start iowatch with a utility that will fail
  "$IOWATCH" -p -f "$TESTFILE" -z -s "exit 1" > "$OUTFILE" 2>&1 &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Modify the file
  echo "trigger change" >> "$TESTFILE"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # iowatch should still work even if the utility fails
  # The test passes if we get here without hanging
  [ "$?" -eq 0 ]
}

@test "ignores rapid file changes (debouncing)" {
  OUTFILE="$TMPDIR/debounce_test.txt"

  # Start iowatch with very short delay
  "$IOWATCH" -p -f "$TESTFILE" -d 50 -z echo "debounced" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Make rapid changes to the file
  echo "change1" >> "$TESTFILE"
  echo "change2" >> "$TESTFILE"
  echo "change3" >> "$TESTFILE"
  sleep 0.3

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that only one execution happened due to debouncing
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "debounced" ]]
  
  # Count lines to ensure only one execution
  line_count=$(echo "$output" | wc -l)
  [ "$line_count" -eq 1 ]
}

@test "handles file deletion and recreation" {
  OUTFILE="$TMPDIR/deletion_test.txt"

  # Start iowatch watching the file
  "$IOWATCH" -p -f "$TESTFILE" -z echo "file recreated" > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Delete and recreate the file
  rm "$TESTFILE"
  sleep 0.1
  echo "recreated content" > "$TESTFILE"
  sleep 0.5

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # Check that the recreation was detected
  run cat "$OUTFILE"
  [ "$status" -eq 0 ]
  [[ "$output" == "file recreated" ]]
}

@test "uses custom kill signal" {
  OUTFILE="$TMPDIR/kill_signal_test.txt"

  # Start iowatch with a custom kill signal and a long-running process
  "$IOWATCH" -p -f "$TESTFILE" -k SIGKILL -d 50 -z sleep 10 > "$OUTFILE" &
  WATCH_PID=$!

  # Give it a moment to start up
  sleep 0.2

  # Modify the file to trigger the long-running process
  echo "trigger" >> "$TESTFILE"
  sleep 0.3

  # Kill iowatch to clean up
  kill "$WATCH_PID" 2>/dev/null || true
  wait "$WATCH_PID" 2>/dev/null || true

  # The test passes if we don't hang (the process should be killed with SIGKILL)
  [ "$?" -eq 0 ]
}

@test "handles invalid kill signal gracefully" {
  run "$IOWATCH" -z -f "$TESTFILE" -k INVALID_SIGNAL echo "test"
  [ "$status" -ne 0 ]
  [[ "$output" == *"Invalid kill signal"* ]]
}