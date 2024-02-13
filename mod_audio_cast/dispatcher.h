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
        mutex mtx_wr;
        list<char *> q;
        int fd;
        bool done = false;
        const char * myfifo = "/tmp/mod-audio-cast-pipe";
        dispatcher();
    public:
        ~dispatcher();
        void dispatch(payload * p);
        void run();
        void stop();
        static dispatcher * get_instance() {
            static dispatcher * instance;
            if (instance == NULL) {
                instance = new dispatcher();
                thread t(&dispatcher::run, instance);
                t.detach();
            }
            return instance;
        }
};


#endif // DISPATCHER_H