#ifndef DISPATCHER_H
#define DISPATCHER_H

#include <queue>
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
        queue<char *> q;
        int fd;
        const char * dir = "/tmp/mod-audio-cast-pipes/";
        char * file_path;
        int batch_size = 5;
        char *batch_buf = nullptr;
        int batch_buf_len = 0;
        void push_to_queue(char * buf);
        int write_to_file(int fd, char * buf);
        char* concat(char* a, size_t a_size,char* b, size_t b_size);
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