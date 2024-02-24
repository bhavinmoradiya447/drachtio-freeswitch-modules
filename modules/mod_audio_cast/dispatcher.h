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
#include <stdlib.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <queue>

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
        int fd;
        queue<payload *> q;
        const char * sock_path = "/tmp/test-mcs-ds.sock";
        struct sockaddr_un remote;
        int batch_size = 1;
        unsigned int seq = 0;
        int batch_buf_len = 0;
        char * call_uuid;
        void write_to_ds(int fd, char * buf, int size);
        void dispatch_to_ds(int size, uuid_t id, int seq, unsigned long timestamp);
    public:
        dispatcher(char * uuid);
        ~dispatcher();
        int connet_ds_socket();
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