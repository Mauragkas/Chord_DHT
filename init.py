#!/usr/bin/env python3
from dotenv import load_dotenv
import os
import sys
import signal
import time
import subprocess
import psutil

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

def kill_proc_tree(pid):
    try:
        parent = psutil.Process(pid)
        children = parent.children(recursive=True)
        for child in children:
            try:
                child.kill()
            except psutil.NoSuchProcess:
                pass
        try:
            parent.kill()
        except psutil.NoSuchProcess:
            pass
    except psutil.NoSuchProcess:
        pass

def monitor_resources(pids):
    # Initialize processes dictionary
    procs = {}
    for pid in pids:
        try:
            proc = psutil.Process(pid)
            procs[pid] = proc
            proc.cpu_percent()  # Initialize CPU monitoring
        except psutil.NoSuchProcess:
            continue

    while True:
        total_cpu = 0
        total_memory = 0

        time.sleep(0.1)

        # Check and update running processes
        active_procs = {}
        for pid, proc in list(procs.items()):
            try:
                # More thorough process status check
                if proc.is_running() and proc.status() != psutil.STATUS_ZOMBIE:
                    # Try to actually get process info to verify it's really running
                    proc.cpu_percent()
                    mem_info = proc.memory_info()

                    active_procs[pid] = proc
                    total_cpu += proc.cpu_percent()
                    total_memory += mem_info.rss / 1024 / 1024
            except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
                continue

        # Update processes dictionary with only active processes
        procs = active_procs

        # Only print if there are active processes
        if procs:
            print(f"\r\033[K"
                  f"No. of Nodes: {len(procs)} "
                  f"Total CPU: {min(total_cpu/len(procs), 100):.1f}% "
                  f"Total Memory: {total_memory:.2f}MB "
                  f"Avg Memory/Node: {total_memory/len(procs):.2f}MB",
                  end='', flush=True)
        else:
            print("\r\033[KAll nodes have stopped.", end='', flush=True)
            break

        # Additional verification of process existence
        procs = {pid: proc for pid, proc in procs.items()
                if psutil.pid_exists(pid) and proc.is_running()}

def signal_handler(sig, frame):
    # Kill all child processes and their children
    for pid in node_pids:
        kill_proc_tree(pid)
    kill_proc_tree(chord_pid)
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

# wait 5 secs
time.sleep(5)

# Start resource monitoring in a separate process
monitor_pid = os.fork()
if monitor_pid == 0:
    monitor_resources(node_pids)
else:
    # Wait for all processes
    os.waitpid(chord_pid, 0)
    for pid in node_pids:
        os.waitpid(pid, 0)
    kill_proc_tree(monitor_pid)
