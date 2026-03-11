#!/bin/bash
set -e

BIN="./target/release/mem"

echo "=== Smoke Tests ==="

# Build first
cargo build --release --quiet

# Test 1: --help
echo -n "Test --help: "
if $BIN --help | grep -q "Consume CPU and memory"; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

# Test 2: dry run (no resources, instant exit)
echo -n "Test dry run: "
output=$($BIN --duration 1 2>&1)
if echo "$output" | grep -q "Consuming: CPU 0%, Memory 0 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

# Test 3: memory formats
echo -n "Test memory formats (2GB): "
output=$($BIN -m 2GB --duration 1 2>&1)
if echo "$output" | grep -q "Memory 2147483648 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test memory formats (512M): "
output=$($BIN -m 512M --duration 1 2>&1)
if echo "$output" | grep -q "Memory 536870912 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test memory formats (1G): "
output=$($BIN -m 1G --duration 1 2>&1)
if echo "$output" | grep -q "Memory 1073741824 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

#!/bin/bash
set -e

BIN="./target/release/mem"

echo "=== Smoke Tests ==="

# Build first
cargo build --release --quiet

# Test 1: --help
echo -n "Test --help: "
if $BIN --help | grep -q "Consume CPU and memory"; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

# Test 2: dry run (no resources, instant exit)
echo -n "Test dry run: "
output=$($BIN --duration 1 2>&1)
if echo "$output" | grep -q "Consuming: CPU 0%, Memory 0 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

# Test 3: memory formats
echo -n "Test memory formats (2GB): "
output=$($BIN -m 2GB --duration 1 2>&1)
if echo "$output" | grep -q "Memory 2147483648 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test memory formats (512M): "
output=$($BIN -m 512M --duration 1 2>&1)
if echo "$output" | grep -q "Memory 536870912 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test memory formats (1G): "
output=$($BIN -m 1G --duration 1 2>&1)
if echo "$output" | grep -q "Memory 1073741824 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

# Test 4: verify actual memory consumption (check delta)
echo -n "Test actual memory (50M): "
$BIN -m 50M --max-memory-percent 20 --duration 20 > /dev/null 2>&1 &
pid=$!

# Wait for memory allocation to complete
sleep 2

echo "Initial memory:"
initial=$(ps -p $pid -o rss= | tr -d ' ')

# Monitor memory for 5 seconds
for i in {1..5}; do
    current=$(ps -p $pid -o rss= | tr -d ' ')
    echo "Second $i: ${current}KB"
    sleep 1
done

wait $pid 2>/dev/null

expected_kb=$((50 * 1024))
min_kb=$((expected_kb * 90 / 100))
if [ "$initial" -ge "$min_kb" ] && [ "$current" -ge "$min_kb" ]; then
    echo "PASS (initial: ${initial}KB, final: ${current}KB)"
else
    echo "FAIL: initial: ${initial}KB, final: ${current}KB"
    exit 1
fi

# Test 5: verify actual CPU consumption (check delta from baseline)
echo -n "Test actual CPU (100%): "
$BIN -c 100 --duration 20 > /dev/null 2>&1 &
pid=$!

# Sample CPU multiple times to verify sustained load
sleep 1
cpu1=$(ps -p $pid -o pcpu= 2>/dev/null | tr -d ' ')
sleep 1
cpu2=$(ps -p $pid -o pcpu= 2>/dev/null | tr -d ' ')
sleep 1
cpu3=$(ps -p $pid -o pcpu= 2>/dev/null | tr -d ' ')
wait $pid 2>/dev/null

min_cpu=90
pass1=$(echo "$cpu1 > $min_cpu" | bc 2>/dev/null || echo "0")
pass2=$(echo "$cpu2 > $min_cpu" | bc 2>/dev/null || echo "0")
pass3=$(echo "$cpu3 > $min_cpu" | bc 2>/dev/null || echo "0")

if [ "$pass1" = "1" ] && [ "$pass2" = "1" ] && [ "$pass3" = "1" ]; then
    avg=$(echo "scale=1; ($cpu1 + $cpu2 + $cpu3) / 3" | bc)
    echo "PASS (samples: ${cpu1}%, ${cpu2}%, ${cpu3}%, avg: ${avg}%)"
else
    echo "FAIL: samples ${cpu1}%, ${cpu2}%, ${cpu3}% (expected all > ${min_cpu}%)"
    exit 1
fi

# Test 6: verify multi-core CPU consumption
echo -n "Test actual CPU (200%): "
$BIN -c 200 --duration 15 > /dev/null 2>&1 &
pid=$!

sleep 1
cpu1=$(ps -p $pid -o pcpu= 2>/dev/null | tr -d ' ')
sleep 1
cpu2=$(ps -p $pid -o pcpu= 2>/dev/null | tr -d ' ')
wait $pid 2>/dev/null

min_cpu=180
pass1=$(echo "$cpu1 > $min_cpu" | bc 2>/dev/null || echo "0")
pass2=$(echo "$cpu2 > $min_cpu" | bc 2>/dev/null || echo "0")

if [ "$pass1" = "1" ] && [ "$pass2" = "1" ]; then
    echo "PASS (samples: ${cpu1}%, ${cpu2}%)"
else
    echo "FAIL: samples ${cpu1}%, ${cpu2}% (expected all > ${min_cpu}%)"
    exit 1
fi

# Test 7: CPU + memory + duration exits cleanly
echo -n "Test combined CPU+memory duration: "
start=$(date +%s)
$BIN -c 50 -m 100M --duration 5 >/dev/null 2>&1
end=$(date +%s)
elapsed=$((end - start))

if [ "$elapsed" -ge 5 ] && [ "$elapsed" -lt 8 ]; then
    echo "PASS (${elapsed}s)"
else
    echo "FAIL: elapsed=${elapsed}s"
    exit 1
fi

# Test 8: empty memory string
echo -n "Test empty memory: "
output=$($BIN -m "" --duration 1 2>&1)
if echo "$output" | grep -q "Memory 0 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

# Test 9: CPU limit enforcement
echo -n "Test CPU limit (9999% capped): "
output=$($BIN -c 9999 -m 0 --duration 1 2>&1)
if echo "$output" | grep -q "capped to 800%"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

# Test 10: memory limit enforcement
echo -n "Test memory limit (999GB capped): "
output=$($BIN -c 0 -m 999GB --duration 1 2>&1)
if echo "$output" | grep -q "exceeds limit, capped"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo ""
echo "=== All tests passed ==="
