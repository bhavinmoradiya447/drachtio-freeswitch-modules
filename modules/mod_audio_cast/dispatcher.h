#ifndef DISPATCHER_H
#define DISPATCHER_H

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
        const char * sock_path = "/tmp/test-mcs-ds.sock";
        struct sockaddr_un remote;
        int batch_size = 1;
        char *batch_buf = nullptr;
        unsigned int seq = 0;
        int batch_buf_len = 0;
        char * call_uuid;
        void write_to_ds(int fd, char * buf);
        char* concat(char* a, size_t a_size,char* b, size_t b_size);
        void dispatch_to_ds(char* buf, int size, uuid_t id, int seq, unsigned long timestamp);
    public:
        dispatcher(char * uuid);
        ~dispatcher();
        int connet_ds_socket();
        void dispatch(payload * p);
        void stop();
};


#endif // DISPATCHER_H