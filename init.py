#!/usr/bin/env python3
from dotenv import load_dotenv
import os
import sys
import signal
import time
import subprocess

load_dotenv()
port = int(os.getenv('PORT') or 3000)
num_nodes = int(os.getenv('NUM_OF_NODES') or 3)

# Get release flag from command line args
release_flag = True if len(sys.argv) > 1 and sys.argv[1] == 'release' else ''

def get_tailscale_ip():
    try:
        # Run the command and capture output
        result = subprocess.run(['tailscale', 'ip'],
                              capture_output=True,
                              text=True,
                              check=True)

        # Split output by lines and take first line, strip whitespace
        ip = result.stdout.splitlines()[0].strip()
        return ip
    except (subprocess.SubprocessError, IndexError) as e:
        # Handle both subprocess errors and potential index errors
        raise IOError("Failed to get Tailscale IP") from e

# Get Tailscale IP
try:
    ip = get_tailscale_ip()
except IOError as e:
    print(e)
    sys.exit(1)


def signal_handler(sig, frame):
    # Kill all child processes
    for pid in node_pids:
        try:
            os.kill(pid, signal.SIGTERM)
        except:
            pass
    try:
        os.kill(chord_pid, signal.SIGTERM)
    except:
        pass
    sys.exit(0)

signal.signal(signal.SIGINT, signal_handler)

# Build first based on release flag
if release_flag:
    subprocess.run(['cargo', 'build', '--release'])
else:
    subprocess.run(['cargo', 'build'])

# Fork and start chord ring process
chord_pid = os.fork()
if chord_pid == 0:
    if release_flag:
        os.execvp('cargo', ['cargo', 'r', '-q', '-r', '--', str(port), 'chord'])
    else:
        os.execvp('cargo', ['cargo', 'r', '-q', '--', str(port), 'chord'])
# time.sleep(1)

# Start node processes
node_pids = []
for i in range(num_nodes):
    node_port = port + i + 1
    node_pid = os.fork()
    if node_pid == 0:
        if release_flag:
            os.execvp('cargo', ['cargo', 'r', '-r', '-q', '--', str(port), 'node', str(node_port), ip])
        else:
            os.execvp('cargo', ['cargo', 'r', '-q', '--', str(port), 'node', str(node_port), ip])
    else:
        node_pids.append(node_pid)

# Wait for all processes
os.waitpid(chord_pid, 0)
for pid in node_pids:
    os.waitpid(pid, 0)
