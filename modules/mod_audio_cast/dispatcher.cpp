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
}

void dispatcher::dispatch_to_ds(char* audio_buf, int size, uuid_t id, int seq, unsigned long timestamp) {
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
    if(size>0) {
        memcpy(buf + pos, audio_buf, size);
        delete[] audio_buf;
    } else {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] queued end of stream for: %s\n", call_uuid);
    }
    write_to_ds(fd, buf);
           
}
void dispatcher::dispatch(payload * p) {
    
    char * buf = new char[p->size];
    memcpy(buf, p->buf, p->size);
            
    if(p->size !=0 ) {
        if(!batch_buf){
            //batch_buf = (char*)realloc(p->buf, p->size);
            batch_buf = (char*)realloc(buf, p->size);
            batch_buf_len = p->size;
        } else {
            batch_buf = concat(batch_buf, batch_buf_len, buf, p->size);
            batch_buf_len = batch_buf_len + p->size;
        }
        if(p->seq % batch_size == 0) {
        
            // fixed size header 32 bytes
            dispatch_to_ds(batch_buf, batch_buf_len, p->id, seq++, p->timestamp);            
            batch_buf = nullptr;
            batch_buf_len =0;
        }
    } else {
        if(batch_buf){
            dispatch_to_ds(batch_buf, batch_buf_len, p->id, seq++, p->timestamp);            
            batch_buf = nullptr;
            batch_buf_len =0;
        }
        dispatch_to_ds(buf, p->size, p->id, seq++, p->timestamp);            
    } 
    //close(fd);
}

char* dispatcher::concat(char* a, size_t a_size, char* b, size_t b_size) {
    char* c = (char*)realloc(a, a_size + b_size);
    memcpy(c + a_size, b,  b_size);  // dest is after "a" data, source is b with b_size
    delete[] b;
    return c;
}



void dispatcher::write_to_ds(int fd, char * buf) {
    int size;
    int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
    int size_pos = 16 + sizeof(int) + sizeof(long);
    memcpy(&size, buf + size_pos, sizeof(int));

    if (sendto(fd, buf, header_size + size, 0, (struct sockaddr *)&remote, sizeof(remote)) == -1) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "Error writing to Domain socket: %s, ERROR: %s\n", call_uuid, strerror(errno));
            connet_ds_socket();
    }
    delete[] buf;
    buf = nullptr;
}

void dispatcher::stop() {
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[INFO] Stop streaming for %s\n", call_uuid);
    if(fd > 0) {
        close(fd);
    }
    
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[INFO] Stopped streaming and closed Domain socket for %s\n", call_uuid);

}


