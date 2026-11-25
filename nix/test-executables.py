"""Read Cargo build messages and print the paths of compiled test executables."""

import json
import sys


for line in sys.stdin:
    message = json.loads(line)
    if message["reason"] == "compiler-message":
        sys.stderr.write(message["message"]["rendered"] or "")
    elif message["reason"] == "compiler-artifact" and message["profile"]["test"]:
        if executable := message["executable"]:
            print(executable)
