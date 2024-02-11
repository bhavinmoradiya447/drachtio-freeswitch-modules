#ifndef CONSISTANCE_HASH_H
#define CONSISTANCE_HASH_H

#include <iostream>
#include <map>
#include <string>
#include <functional>

using namespace std;


class consistence_hashing {
public:
    // Constructor to initialize the number of virtual nodes
    consistence_hashing(int virtualNodes);
    // Add a node to the consistent hashing ring
    void addNode(const string &node);
    // Remove a node from the consistent hashing ring
    void removeNode(const string &node) ;
    // Get the node responsible for a given key
    string getNode(const string &key);

private:
    int virtualNodes_;  // Number of virtual nodes per real node
    map<size_t, string> ring_;
};
#endif // CONSISTANCE_HASH_H