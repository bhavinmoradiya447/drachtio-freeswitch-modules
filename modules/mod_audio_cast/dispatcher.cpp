#include "dispatcher.h"

dispatcher::dispatcher(char * uuid) {
    file_path = switch_mprintf("%s%s", dir, uuid);
    mkfifo(file_path, 0666);
    fd = open(file_path, O_WRONLY | O_NONBLOCK);
}

dispatcher::~dispatcher() {
    if(fd>0){
        close(fd);
    }
    // unlink(myfifo);
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
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] queued end of stream for file: %s\n", file_path);
    }

    if(p->seq % batch_size !=0 && p->size !=0 ) {
        if(!batch_buf){
            batch_buf = (char*)realloc(buf, len);
            batch_buf_len = len;
        } else {
            batch_buf = concat(batch_buf, batch_buf_len, buf, len);
            batch_buf_len = batch_buf_len + len;
        }
    } else {
         if(!batch_buf){
            batch_buf = (char*)realloc(buf, len);
            batch_buf_len = len;
        } else {
            batch_buf = concat(batch_buf, batch_buf_len, buf, len);
            batch_buf_len = batch_buf_len + len;
        }

        char* final_buf = new char[ batch_buf_len + sizeof(int)];

        memcpy(final_buf, &batch_buf_len, sizeof(int));
        memcpy(final_buf+sizeof(int), batch_buf, batch_buf_len);


        if(fd < 0){
                fd = open(file_path, O_WRONLY | O_NONBLOCK);
        }

        if(fd < 0) {
            //
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Unable to open named pipe: %s, Error: %s\n", file_path, strerror(errno));
            push_to_queue(final_buf);
        } else {
            while(!q.empty()){
                char * queued_buf = q.front();
                int status = write_to_file(fd, queued_buf);
                if(status < 0) {
                    break;
                }
                q.pop();            
            }
            int status = write_to_file(fd, final_buf);
            if(status < 0) {
                push_to_queue(final_buf);
            }
        }
        batch_buf = nullptr;
        batch_buf_len =0;
    }


   
    //close(fd);
}

char* dispatcher::concat(char* a, size_t a_size, char* b, size_t b_size) {
    char* c = (char*)realloc(a, a_size + b_size);
    memcpy(c + a_size, b,  b_size);  // dest is after "a" data, source is b with b_size
    free(b);
    return c;
}


void dispatcher::push_to_queue(char * buf) {
    if(q.size() > QUEUE_MAX_SIZE) {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] queue for %s is fulled, ignoring audio stream\n", file_path);
        delete[] buf;
        return;
    } else {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[DEBUG] Pushing to Queue, Queue Size :%d\n", q.size());
        q.push(buf);        
    }
}

int dispatcher::write_to_file(int fd, char * buf) {
    int size;
    memcpy(&size, buf, sizeof(int));
    int ret = write(fd, buf + sizeof(int), size);
    if (ret < 0)
    {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "Error writing to pipe: %s, ERROR: %s\n", file_path, strerror(errno));
        return -1;
    }
    fsync(fd);
    delete[] buf;
    buf = nullptr;
    return 1;
}

/*
void dispatcher::run() {
    while (true) {
        unique_lock<mutex> lck(mtx);
        // cout << "dispatcher waiting to read" << endl;
        if (q.empty()) {
            cv.wait(lck, [this]{return ready || done;});
        }   
        // cv.wait(lck, [this]{return ready || done;});
        if(done) {
            cout << "dispatcher done" << endl;
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
            cout << "Error writing to pipe" << endl;
            return;
        }
        else 
        {
            //cout << "[info] sent end of stream" << endl;
            // flush
            fsync(fd);
        }
        delete[] buf;
        processed = true;
    }
}
*/
void dispatcher::stop() {
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[INFO] Stop streaming and closing file %s\n", file_path);

    if(fd < 0){
        fd = open(file_path, O_WRONLY | O_NONBLOCK);
    }
     if(fd>0){
         while(!q.empty()){
            char * queued_buf = q.front();
            q.pop();
            int status = write_to_file(fd, queued_buf);
            if(status < 0) {
                switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Failed to write data on stop for file %s\n", file_path);
            }
            delete[] queued_buf;
        }
        close(fd);
    } else {
        char * queued_buf = q.front();
        q.pop();
        delete[] queued_buf;
    }
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[INFO] Stopped streaming and closed file %s\n", file_path);

}


