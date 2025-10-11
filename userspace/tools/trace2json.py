#!/usr/bin/env python3
import sys, json, time
for line in sys.stdin:
    print(json.dumps({"ts_ns": int(time.time()*1e9), "type": "raw", "line": line.strip()}))
