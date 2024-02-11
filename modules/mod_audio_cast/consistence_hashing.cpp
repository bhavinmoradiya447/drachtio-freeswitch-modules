#include <consistence_hashing.h>

// Define a simple hash function using hash
size_t hash_fn(const string &key) {
    hash<string> hasher;
    return hasher(key);
}


consistence_hashing::consistence_hashing(int virtualNodes) : virtualNodes_(virtualNodes) {}

// Add a node to the consistent hashing ring
void consistence_hashing::addNode(const string &node) {
    for (int i = 0; i < virtualNodes_; ++i) {
        string virtualNodeName = node + "-" + to_string(i);
        size_t hash = hash_fn(virtualNodeName);
        ring_[hash] = node;
    }
}

// Remove a node from the consistent hashing ring
void consistence_hashing::removeNode(const string &node) {
    for (int i = 0; i < virtualNodes_; ++i) {
        string virtualNodeName = node + "-" + to_string(i);
        size_t hash = hash_fn(virtualNodeName);
        ring_.erase(hash);
    }
}

// Get the node responsible for a given key
string consistence_hashing::getNode(const string &key) {
    if (ring_.empty()) {
        return "";
    }

    size_t hash = hash_fn(key);
    auto it = ring_.lower_bound(hash);

    if (it == ring_.end()) {
        // Wrap around if the key's hash is greater than the largest hash in the ring
        it = ring_.begin();
    }

    return it->second;
}

