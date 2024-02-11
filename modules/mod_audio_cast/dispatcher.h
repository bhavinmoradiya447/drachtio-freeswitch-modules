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

#define POOL_SIZE  5 // 


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
        mutex mtx_wr_arr[POOL_SIZE];
        list<char *> q_arr[POOL_SIZE];
        bool done_arr[POOL_SIZE];
        int fd_arr[POOL_SIZE];
        consistence_hashing * consistance_hash;
        const char * fifo_file_prefix = "/tmp/mod-audio-cast-pipe";
        dispatcher();
    public:
        ~dispatcher();
        void dispatch(payload * p, char * uuid);
        void run(int index);
        void stop();
        static dispatcher * get_instance() {
            static dispatcher * instance;
            if (instance == NULL) {
                instance = new dispatcher();
                for(int i=0; i< POOL_SIZE ; i++){
                    thread t(&dispatcher::run, instance, i);
                    t.detach();
                }
            }
            return instance;
        }
};


#endif // DISPATCHER_H