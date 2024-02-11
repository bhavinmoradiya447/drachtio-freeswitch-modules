#include "dispatcher.h"
#include <errno.h>


dispatcher::dispatcher() {
    mkfifo(myfifo, 0666);
    fd = open(myfifo, O_WRONLY);
}

dispatcher::~dispatcher() {
    close(fd);
    unlink(myfifo);
}

// void dispatcher::dispatch(uuid_t id, char * buf, unsigned int size, unsigned int seq) {
//     unsigned int len = 16 + (2 * sizeof(int)) + size;
//     int pos = 0;
//     char * buf_copy = new char[len];
//     memcpy(buf_copy + pos, &len, sizeof(int));
//     pos = pos + sizeof(int);
//     memcpy(buf_copy + pos, &id, 16);
//     pos = pos + 16;
//     memcpy(buf_copy + pos, &seq, sizeof(int));
//     pos = pos + sizeof(int);
//     if (size > 0) 
//     {
//         memcpy(buf_copy + pos, buf, size);
//     }
//     unique_lock<mutex> lck(mtx);
//     q.push(buf_copy);
//     ready = true;
//     cv.notify_one();
// }

void dispatcher::dispatch(payload * p) {
    // fixed size header 32 bytes
    int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
    // compute buffer size
    unsigned int len = header_size + p->size;

    // create buffer
    char * buf = new char[len];
    int pos = 0;
    // copy uuid to buffer
    memcpy(buf + pos, &p->id, 16);
    pos = pos + 16;
    // copy seq to buffer
    memcpy(buf + pos, &p->seq, sizeof(int));
    pos = pos + sizeof(int);
    // copy timestamp to buffer
    memcpy(buf + pos, &p->timestamp, sizeof(long));
    pos = pos + sizeof(long);
    // copy size to buffer
    memcpy(buf + pos, &p->size, sizeof(int));
    pos = pos + sizeof(int);
    // copy payload to buffer
    if (p->size > 0)
    {
        memcpy(buf + pos, p->buf, p->size);
    } else {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] queued end of stream:");
    }
    unique_lock<mutex> lck(mtx);
    q.push(buf);
    ready = true;
    cv.notify_one();
}

void dispatcher::run() {
    while (true) {
        unique_lock<mutex> lck(mtx);
        // cout << "dispatcher waiting to read" << endl;
        if (q.empty()) {
            cv.wait(lck, [this]{return ready || done;});
        }   
        // cv.wait(lck, [this]{return ready || done;});
        if(done) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] dispatcher done");
            close(fd);
            break;
        }
        // cout << "dispatcher read" << endl;
        char * buf = q.front();
        q.pop();
        ready = false;
        lck.unlock();
        // read size from buf
        int size;
        int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
        int size_pos = 16 + sizeof(int) + sizeof(long);
        memcpy(&size, buf + size_pos, sizeof(int));
        int ret = write(fd, buf, header_size + size);
        if (ret < 0)
        {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Error writing to pipe: %s\n", strerror(errno));
            goto end:
        } else if (ret < header_size + size) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Partial Write happend, expteded %d but wrote only %d", (header_size + size), ret);

        }
        else 
        {
            //cout << "[info] sent end of stream" << endl;
            // flush
            fsync(fd);
        }
        end:
        delete[] buf;
        processed = true;
    }
}

void dispatcher::stop() {
    unique_lock<mutex> lck(mtx);
    done = true;
    cv.notify_all();
}