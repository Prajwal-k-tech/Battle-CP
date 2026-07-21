#!/bin/bash
# ============================================================
# Battle CP - Oracle Cloud Anti-Idle Script
# Prevents Oracle from reclaiming idle A1 Flex instances
# Oracle reclaims if CPU+network+memory are all <20% over 7 days
#
# Install via crontab:
#   */5 * * * * /path/to/oracle_anti_idle.sh > /dev/null 2>&1
# ============================================================
set -euo pipefail

# Config
THRESHOLD=20
PIDFILE="/tmp/oracle_anti_idle.pid"
KEEPALIVE_FILE="/dev/shm/battlecp_keepalive"

# Prevent concurrent runs (flock-style)
if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
    # Previous run still active, skip
    exit 0
fi
echo $$ > "$PIDFILE"
trap 'rm -f "$PIDFILE" "$KEEPALIVE_FILE"' EXIT

# ---- Get CPU usage from /proc/stat (locale-safe, low overhead) ----
read -r _ user nice system idle iowait irq softirq steal < /proc/stat
total_old=$((user + nice + system + idle + iowait + irq + softirq + steal))
idle_old=$((idle + iowait))

sleep 0.5

read -r _ user nice system idle iowait irq softirq steal < /proc/stat
total_new=$((user + nice + system + idle + iowait + irq + softirq + steal))
idle_new=$((idle + iowait))

total_delta=$((total_new - total_old))
idle_delta=$((idle_new - idle_old))

if [ "$total_delta" -gt 0 ]; then
    cpu_usage=$(( (100 * (total_delta - idle_delta)) / total_delta ))
else
    cpu_usage=0
fi

# ---- If CPU is below threshold, generate some activity ----
if [ "$cpu_usage" -lt "$THRESHOLD" ]; then
    # Write to memory (will be reclaimed if RAM is needed) to show memory usage
    dd if=/dev/zero of="$KEEPALIVE_FILE" bs=1M count=256 2>/dev/null
    
    # CPU burn: compute sha256 hashes for ~30s
    end=$((SECONDS + 30))
    while [ $SECONDS -lt $end ]; do
        sha256sum /proc/stat > /dev/null 2>&1
    done
    
    # Clean up memory file
    rm -f "$KEEPALIVE_FILE"
fi
