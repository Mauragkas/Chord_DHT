import hashlib
import threading
import time
from typing import Optional, Dict, List

class Node:
    def __init__(self, node_id: int, m: int):
        """Initialize a Chord node with a given ID and address space size."""
        self.node_id = node_id
        self.m = m  # size of address space
        self.successor = self
        self.predecessor: Optional[Node] = None
        self.finger_table: List[Node] = [self] * m
        self.data: Dict[str, str] = {}
        self.running = True
        self.lock = threading.Lock()  # Add thread safety

    def hash_function(self, key: str) -> int:
        """Hash a key to get its identifier in the Chord ring."""
        return int(hashlib.sha1(str(key).encode()).hexdigest(), 16) % (2 ** self.m)

    def in_range(self, key: int, start: int, end: int) -> bool:
        """Check if a key falls within a range on the Chord ring."""
        if start <= end:
            return start <= key <= end
        return start <= key or key <= end

    def find_successor(self, id: int) -> 'Node':
        """Find the successor node for a given identifier."""
        print(f"Node {self.node_id}: Finding successor for id {id}")
        with self.lock:
            if self.successor is None:
                return self

            # Check if id is between current node and its successor
            if self.in_range(id, (self.node_id + 1) % (2 ** self.m), self.successor.node_id):
                print(f"Node {self.node_id}: Successor for id {id} is {self.successor.node_id}")
                return self.successor

            # Find the closest preceding node
            n = self.closest_preceding_node(id)
            if n == self:
                print(f"Node {self.node_id}: Closest preceding node for id {id} is self")
                return self.successor
            print(f"Node {self.node_id}: Closest preceding node for id {id} is {n.node_id}")
            return n.find_successor(id)

    def closest_preceding_node(self, id: int) -> 'Node':
        """Find the closest preceding node for a given identifier."""
        print(f"Node {self.node_id}: Finding closest preceding node for id {id}")
        with self.lock:
            for i in range(self.m - 1, -1, -1):
                if self.finger_table[i] and self.in_range(
                    self.finger_table[i].node_id,
                    (self.node_id + 1) % (2 ** self.m),
                    (id - 1) % (2 ** self.m)
                ):
                    print(f"Node {self.node_id}: Closest preceding node for id {id} is {self.finger_table[i].node_id}")
                    return self.finger_table[i]
            print(f"Node {self.node_id}: No closest preceding node found for id {id}, returning self")
            return self

    def join(self, node: Optional['Node']) -> None:
        """Join the Chord ring using an existing node."""
        with self.lock:
            if node:
                self.predecessor = None
                self.successor = node.find_successor(self.node_id)
                # Transfer keys that belong to this node
                self._transfer_keys()
            else:
                self.predecessor = self
                self.successor = self

    def _transfer_keys(self) -> None:
        """Transfer keys that belong to this node from its successor."""
        if self.successor and self.successor != self:
            keys_to_transfer = []
            for key in self.successor.data.keys():
                key_id = self.hash_function(key)
                if self.in_range(key_id, self.predecessor.node_id if self.predecessor else 0, self.node_id):
                    keys_to_transfer.append(key)

            for key in keys_to_transfer:
                self.data[key] = self.successor.data[key]
                del self.successor.data[key]

    def stabilize(self) -> None:
        """Periodically verify node's immediate successor and tell successor about node."""
        with self.lock:
            if not self.successor or not self.successor.is_alive():
                # If successor is dead, try to recover using finger table
                for i in range(self.m):
                    if self.finger_table[i] and self.finger_table[i].is_alive():
                        self.successor = self.finger_table[i]
                        break
                return

            x = self.successor.predecessor
            if x and x.is_alive() and self.in_range(
                x.node_id,
                (self.node_id + 1) % (2 ** self.m),
                (self.successor.node_id - 1) % (2 ** self.m)
            ):
                self.successor = x
            self.successor.notify(self)

    def notify(self, node: 'Node') -> None:
        """Handle notification from another node that it thinks it's our predecessor."""
        with self.lock:
            if (not self.predecessor or not self.predecessor.is_alive() or
                self.in_range(node.node_id, (self.predecessor.node_id + 1) % (2 ** self.m), (self.node_id - 1) % (2 ** self.m))):
                self.predecessor = node

    def fix_fingers(self) -> None:
        """Periodically refresh finger table entries."""
        with self.lock:
            for i in range(self.m):
                next_finger = (self.node_id + 2 ** i) % (2 ** self.m)
                self.finger_table[i] = self.find_successor(next_finger)

    def check_predecessor(self) -> None:
        """Periodically check if predecessor has failed."""
        with self.lock:
            if self.predecessor and not self.predecessor.is_alive():
                self.predecessor = None

    def insert(self, key: str, value: str) -> None:
        """Insert a key-value pair into the DHT."""
        print(f"Inserting key: {key}, value: {value}")
        id = self.hash_function(key)
        print(f"Hashed key: {id}")
        node = self.find_successor(id)
        print(f"Successor node for key {key}: {node.node_id}")
        with node.lock:
            node.data[key] = value
        print(f"Inserted key: {key}, value: {value} at node {node.node_id}")

    def search(self, key: str) -> Optional[str]:
        """Search for a value by key in the DHT."""
        id = self.hash_function(key)
        node = self.find_successor(id)
        with node.lock:
            return node.data.get(key)

    def delete(self, key: str) -> None:
        """Delete a key-value pair from the DHT."""
        id = self.hash_function(key)
        node = self.find_successor(id)
        with node.lock:
            if key in node.data:
                del node.data[key]

    def leave(self) -> None:
        """Gracefully leave the Chord ring."""
        with self.lock:
            self.running = False
            if self.predecessor and self.successor:
                # Transfer keys to successor
                self.successor.data.update(self.data)
                # Update pointers
                self.predecessor.successor = self.successor
                self.successor.predecessor = self.predecessor

    def print_ring(self) -> None:
        """Print the current state of the Chord ring."""
        visited = set()
        current = self
        while current and current.node_id not in visited:
            print(f"Node {current.node_id}: {current.data}")
            visited.add(current.node_id)
            current = current.successor
            if current == self:
                break

    def is_alive(self) -> bool:
        """Check if the node is still active."""
        return self.running

def create_ring(m: int) -> Node:
    """Create a new Chord ring with a single node."""
    node = Node(0, m)
    node.join(None)
    return node

def add_node_to_ring(existing_node: Node, new_node_id: int) -> Node:
    """Add a new node to an existing Chord ring."""
    new_node = Node(new_node_id, existing_node.m)
    new_node.join(existing_node)
    return new_node

def run_stabilization(node: Node) -> None:
    """Run the stabilization protocol for a node."""
    while node.is_alive():
        try:
            node.stabilize()
            node.fix_fingers()
            node.check_predecessor()
            time.sleep(1)
        except Exception as e:
            print(f"Error in stabilization for node {node.node_id}: {e}")

if __name__ == "__main__":
    # Test the implementation
    m = 3  # Number of bits in the identifier space
    ring = create_ring(m)

    # Add nodes to the ring
    nodes = [ring]
    for i in range(1, 4):
        node = add_node_to_ring(ring, i)
        nodes.append(node)

    # Start stabilization threads
    threads = []
    for node in nodes:
        thread = threading.Thread(target=run_stabilization, args=(node,))
        thread.daemon = True  # Make threads daemon so they exit when main thread exits
        thread.start()
        threads.append(thread)

    # Test basic operations
    try:
        # Insert data
        print("Inserting data...")
        ring.insert("key1", "value1")
        ring.insert("key2", "value2")

        # Search data
        print("\nSearching for data...")
        print(f"Search key1: {ring.search('key1')}")
        print(f"Search key2: {ring.search('key2')}")

        # Print ring state
        print("\nInitial ring state:")
        ring.print_ring()

        # Delete data
        print("\nDeleting key1...")
        ring.delete("key1")
        print(f"Search key1 after deletion: {ring.search('key1')}")

        # Print final ring state
        print("\nFinal ring state:")
        ring.print_ring()

        # Test node departure
        print("\nTesting node departure...")
        nodes[1].leave()
        time.sleep(2)  # Allow time for stabilization
        print("\nRing state after node departure:")
        ring.print_ring()

    except KeyboardInterrupt:
        print("\nShutting down...")
        for node in nodes:
            node.running = False

        for thread in threads:
            thread.join(timeout=1)
