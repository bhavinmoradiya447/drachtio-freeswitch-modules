#include "dispatcher.h"

dispatcher::dispatcher(char * uuid) {
    char *audio_buf_batch_size = std::getenv("MOD_AUDIO_CAST_BATCH_SIZE");
    batch_size = std::max(1, (audio_buf_batch_size ? ::atoi(audio_buf_batch_size) : 1));
    call_uuid = uuid;
}

int dispatcher::connet_ds_socket() {
    if ((fd = socket(AF_UNIX, SOCK_DGRAM, 0)) == -1) {
        perror("socket creation error");
        return -1;
    }
    memset(&remote, 0, sizeof(remote));
    remote.sun_family = AF_UNIX;
    strcpy(remote.sun_path, sock_path);
    return 1;
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

void dispatcher::dispatch_to_ds(int size, uuid_t id, int seq, unsigned long timestamp) {
    int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
    // compute buffer size
    unsigned int len = header_size + size;
    // create buffer
    char * buf = new char[len];
    int pos = 0;
    // copy uuid to buffer
    memcpy(buf + pos, id, 16);
    pos = pos + 16;
    // copy seq to buffer
    memcpy(buf + pos, &seq, sizeof(int));
    pos = pos + sizeof(int);
    // copy timestamp to buffer
    memcpy(buf + pos, &timestamp, sizeof(long));
    pos = pos + sizeof(long);
    // copy size to buffer
    memcpy(buf + pos, &size, sizeof(int));
    pos = pos + sizeof(int);
    // copy payload to buffer
    while(!q.empty()){
        payload * p = q.front();
        q.pop();
        memcpy(buf + pos, p->buf, p->size);
        pos= pos + p->size;
        if(p->size == 0) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] queued end of stream for: %s\n", call_uuid);
        }
        delete p;        
    }
    write_to_ds(fd, buf, len);          
}
void dispatcher::dispatch(payload * p) {
            
    if(p->size != 0 ) {
        q.push(p);
        batch_buf_len = batch_buf_len + p->size;
        if(q.size() == batch_size) {
            dispatch_to_ds(batch_buf_len, p->id, seq++, p->timestamp);
            batch_buf_len = 0;
        }
    } else {
        if(!q.empty()){
            payload * last = q.back();
            dispatch_to_ds(batch_buf_len, last->id, seq++, last->timestamp); // write final chunk 
            batch_buf_len = 0;
        }
        q.push(p);
        dispatch_to_ds(batch_buf_len, p->id, seq++, p->timestamp);            
    } 
}


void dispatcher::write_to_ds(int fd, char * buf, int size) {
    if (sendto(fd, buf, size, 0, (struct sockaddr *)&remote, sizeof(remote)) == -1) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "Error writing to Domain socket: %s, ERROR: %s\n", call_uuid, strerror(errno));
            connet_ds_socket();
    }
    //fsync(fd);
    delete[] buf;
    buf = nullptr;
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
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[INFO] Stop streaming for %s\n", call_uuid);
    if(fd > 0) {
        close(fd);
    }
    
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[INFO] Stopped streaming and closed Domain socket for %s\n", call_uuid);

}


