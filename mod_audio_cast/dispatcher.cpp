#include "dispatcher.h"

dispatcher::dispatcher() {
    mkfifo(myfifo, 0666);
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
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[Info] queued end of stream\n");
    }
    {
        lock_guard<mutex> lck(mtx);
        q.push_back(buf);
    }
}

void dispatcher::run() {
    while (true) {
        // cout << "dispatcher waiting to read" << endl;
       bool is_empty = false; 
        char * buf;
        int size;
        int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
        int size_pos = 16 + sizeof(int) + sizeof(long);
        {
            lock_guard<mutex> lck(mtx);
            if (q.empty()) {
                is_empty = true;
            } else {
                buf = q.front();
                q.pop_front();
            }
        }
        if(is_empty){
            std::this_thread::sleep_for(chrono::microseconds(100));
            continue; 
        }
        // cout << "dispatcher read" << endl;
       
        // read size from buf
        memcpy(&size, buf + size_pos, sizeof(int));
        {            
            lock_guard<mutex> lck(mtx_wr);
            fd = open(myfifo, O_WRONLY);

            int ret = write(fd, buf, header_size + size);
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[Info] writing to pipe %d\n",strlen(buf));
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[Info] Actule lenght wrote : %d\n",ret);

            if (ret < 0)
            {
                switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Error writing to pipe %s\n", strerror(errno));
                goto end;
            } else if (ret < header_size + size) {
                switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Partial Write happend on named pipe, expteded %d but wrote only %d\n", (header_size + size), ret);
            }
           
                //cout << "[info] sent end of stream" << endl;
                // flush
                close(fd);
            
        }
        end:
        delete[] buf;
        if(done){
            break;
        }
    }
}

void dispatcher::stop() {
    done = true;
}