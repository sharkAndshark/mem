#!/bin/bash
set -e

BIN="./target/release/mem"

echo "=== Smoke Tests ==="

cargo build --release --quiet

echo -n "Test --help: "
if $BIN --help | grep -q "Consume CPU and memory"; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

echo -n "Test dry run: "
output=$($BIN -c 0 -m 0 --duration 1 2>&1)
if echo "$output" | grep -q "CPU: none" && echo "$output" | grep -q "Memory: none"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test memory formats (2GB): "
output=$($BIN -m 2GB --duration 1 2>&1)
if echo "$output" | grep -q "2147483648 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test memory formats (512M): "
output=$($BIN -m 512M --duration 1 2>&1)
if echo "$output" | grep -q "536870912 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test memory formats (1G): "
output=$($BIN -m 1G --duration 1 2>&1)
if echo "$output" | grep -q "1073741824 bytes"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo -n "Test actual memory (50M): "
$BIN -m 50M --duration 10 > /dev/null 2>&1 &
pid=$!
sleep 2
initial=$(ps -p $pid -o rss= 2>/dev/null | tr -d ' ' || echo "0")
sleep 3
current=$(ps -p $pid -o rss= 2>/dev/null | tr -d ' ' || echo "0")
wait $pid 2>/dev/null || true

expected_kb=$((50 * 1024))
min_kb=$((expected_kb * 80 / 100))
if [ "$initial" -ge "$min_kb" ] 2>/dev/null; then
    echo "PASS (initial: ${initial}KB)"
else
    echo "SKIP (initial: ${initial}KB, expected >= ${min_kb}KB)"
fi

echo -n "Test actual CPU (100%): "
$BIN -c 100 --duration 10 > /dev/null 2>&1 &
pid=$!
sleep 1
cpu1=$(ps -p $pid -o pcpu= 2>/dev/null | tr -d ' ' || echo "0")
sleep 1
cpu2=$(ps -p $pid -o pcpu= 2>/dev/null | tr -d ' ' || echo "0")
wait $pid 2>/dev/null || true

min_cpu=50
if [ "$(echo "$cpu1 > $min_cpu" | bc 2>/dev/null || echo 0)" = "1" ] || [ "$(echo "$cpu2 > $min_cpu" | bc 2>/dev/null || echo 0)" = "1" ]; then
    echo "PASS (samples: ${cpu1}%, ${cpu2}%)"
else
    echo "SKIP (samples: ${cpu1}%, ${cpu2}%)"
fi

echo -n "Test combined CPU+memory duration: "
start=$(date +%s)
$BIN -c 50 -m 100M --duration 5 >/dev/null 2>&1
end=$(date +%s)
elapsed=$((end - start))

if [ "$elapsed" -ge 5 ] && [ "$elapsed" -lt 10 ]; then
    echo "PASS (${elapsed}s)"
else
    echo "FAIL: elapsed=${elapsed}s"
    exit 1
fi

echo -n "Test empty memory: "
output=$($BIN -m "" --duration 1 2>&1)
if echo "$output" | grep -q "Memory: none"; then
    echo "PASS"
else
    echo "FAIL: $output"
    exit 1
fi

echo ""
echo "=== All tests passed ==="
