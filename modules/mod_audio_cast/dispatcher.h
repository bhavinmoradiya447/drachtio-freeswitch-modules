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
#include <switch.h>

using namespace std;
#define QUEUE_MAX_SIZE 3000

struct payload {
    uuid_t id;
    unsigned int seq;
    unsigned long timestamp;
    unsigned int size;
    char * buf;
};

class dispatcher {
    private:
        list<char *> q;
        int fd;
        const char * dir = "/tmp/mod-audio-cast-pipes/";
        char * file_path;
        void push_to_queue(char * buf,  bool push_to_front);
        int write_to_file(int fd, char * buf);
    public:
        dispatcher(char * uuid);
        ~dispatcher();
        void dispatch(payload * p);
        //void run();
        void stop();
        /*static dispatcher * get_instance() {
            static dispatcher * instance;
            if (instance == NULL) {
                instance = new dispatcher();
                thread t(&dispatcher::run, instance);
                t.detach();
            }
            return instance;
        }*/
};


#endif // DISPATCHER_H