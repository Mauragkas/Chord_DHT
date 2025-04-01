# Chord DHT Implementation

A Rust implementation of the Chord Distributed Hash Table (DHT) protocol with a web-based monitoring interface.

## Overview

This project implements a Chord DHT system, which is a protocol and algorithm for a peer-to-peer distributed hash table. It allows for efficient location of data in a distributed system without requiring a central server.

Key features:
- Distributed hash table functionality
- Web-based monitoring dashboard
- Real-time node status updates
- Automatic node failure detection
- REST API for data operations

## Requirements

- Rust (latest stable version)
- Python 3.x
- Tailscale (for distributed deployment)
- Required Python packages:
  - python-dotenv
  - psutil

## Configuration

Create a `.env` file in the project root:

```env
M=6                    # Size of the identifier space (2^M)
N=3                    # Number of successor nodes to maintain
PORT=3000             # Base port for the Chord ring
NUM_OF_NODES=3        # Number of nodes to start
DEFAULT_CHANNEL_SIZE=1000  # Channel size for async communication
```

## Running the System

1. Start the system using the initialization script:

```bash
# Debug mode
python3 init.py

# Release mode (optimized)
python3 init.py release
```

2. Access the web interfaces:
- Chord Ring Dashboard: `http://<ip>:3000`
- Individual Node Dashboards: `http://<ip>:300[1-N]`

## Web Interface Features

### Chord Ring Dashboard
- Real-time node monitoring
- Active nodes list
- System logs
- Data upload functionality
- Key lookup interface

### Node Dashboard
- Node information (ID, predecessor, successor)
- Finger table
- Local data storage
- System logs
- Node join/leave controls

## Operations

- **Join**: Nodes automatically join the ring through the Chord protocol
- **Leave**: Nodes can gracefully leave the ring using the web interface
- **Data Upload**: Upload CSV files through the Chord Ring dashboard
- **Lookup**: Search for specific keys in the DHT
- **KYS (Kill Your Self)**: Force terminate a node for testing failure scenarios

## Monitoring

The initialization script provides real-time monitoring of:
- Number of active nodes
- CPU usage
- Memory usage per node
- Total system memory usage

## Implementation Details

The system is implemented in Rust using:
- Actix-web for HTTP server
- Tokio for async runtime
- SQLite for local storage
- Successor lists
- Finger tables for efficient routing
