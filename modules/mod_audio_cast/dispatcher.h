#ifndef DISPATCHER_H
#define DISPATCHER_H

#include <list>
#include <mutex>
#include <condition_variable>
#include <iostream>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>
#include <cstring>
#include <uuid/uuid.h>
#include <thread>
#include <switch.h>
#include "consistence_hashing.h"


#define QUEUE_MAX_SIZE  20000  // i.e 20000 * (8192  byte (Audio packets) + 32 header)  ~ 165 MB 

#define POOL_SIZE  5 // i.e 20000 * (8192  byte (Audio packets) + 32 header)  ~ 165 MB 

using namespace std;

struct payload {
    uuid_t id;
    unsigned int seq;
    unsigned long timestamp;
    unsigned int size;
    char * buf;
};

class dispatcher {
    private:
        mutex mtx_arr[POOL_SIZE];
        condition_variable cv;
        list<char *> q_arr[POOL_SIZE];
        bool ready = false;
        bool processed = false;
        bool done = false;
        int fd;
        const char * fifo_files[POOL_SIZE] ;
        consistence_hashing * consistance_hash;
        dispatcher();
    public:
        ~dispatcher();
        void dispatch(char * buf, char * uuid);
        int write_to_file(int fd, char * buf);
        void push_to_queue(char * buf, int index);
        void stop();
        static dispatcher * get_instance() {
            static dispatcher * instance;
            if (instance == NULL) {
                instance = new dispatcher();
                //thread t(&dispatcher::run, instance);
                //t.detach();
            }
            return instance;
        }
};


#endif // DISPATCHER_H