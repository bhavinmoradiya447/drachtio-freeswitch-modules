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

#define QUEUE_MAX_SIZE  20000  // i.e 20000 * (8192  byte (Audio packets) + 32 header)  ~ 165 MB 

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
        mutex mtx;
        condition_variable cv;
        list<char *> q;
        bool ready = false;
        bool processed = false;
        bool done = false;
        int fd;
        const char * myfifo = "/tmp/mod-audio-cast-pipe";
        dispatcher();
    public:
        ~dispatcher();
        void dispatch(payload * p);
        void write_to_file(int fd, char * buf);
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