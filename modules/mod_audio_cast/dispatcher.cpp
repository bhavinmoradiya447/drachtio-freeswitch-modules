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

void dispatcher::dispatch_to_ds(char* audio_buf, int size, uuid_t id, int seq, unsigned long timestamp,  char* left_buf, int left_size, char* right_buf, int right_size) {
    int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int) + sizeof(int) + sizeof(int);
    // compute buffer size
    unsigned int len = header_size + size + left_size + right_size;

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
   
    memcpy(buf + pos, &left_size, sizeof(int));
    pos = pos + sizeof(int);
    
    memcpy(buf + pos, &right_size, sizeof(int));
    pos = pos + sizeof(int);
    
    // copy payload to buffer
    if(size>0) {
        memcpy(buf + pos, audio_buf, size);
        pos = pos + size;
        memcpy(buf + pos, left_buf, left_size);
        pos = pos + left_size;
        memcpy(buf + pos, right_buf, right_size);
        pos = pos + right_size;
        delete[] audio_buf;
        delete[] left_buf;
        delete[] right_buf;
    } else {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] queued end of stream for: %s\n", call_uuid);
    }
    write_to_ds(fd, buf, pos);
           
}
void dispatcher::dispatch(payload * p) {
    
    char * buf = new char[p->size];
    memcpy(buf, p->buf, p->size);

    char * left_buf = new char[p->left_size];
    memcpy(left_buf, p->left_buf, p->left_size);

    char * right_buf = new char[p->right_size];
    memcpy(right_buf, p->right_buf, p->right_size);

    if(p->size !=0 ) {
        if(!batch_buf){
            batch_buf = (char*)realloc(buf, p->size);
            batch_buf_len = p->size;
            
            batch_left_buf = (char*)realloc(left_buf, p->left_size);
            batch_left_buf_len = p->left_size;
            
            batch_right_buf = (char*)realloc(right_buf, p->right_size);
            batch_right_buf_len = p->right_size;
            
        } else {
            batch_buf = concat(batch_buf, batch_buf_len, buf, p->size);
            batch_buf_len = batch_buf_len + p->size;

            batch_left_buf = concat(batch_left_buf, batch_left_buf_len, left_buf, p->left_size);
            batch_left_buf_len = batch_left_buf_len + p->left_size;

            batch_right_buf = concat(batch_right_buf, batch_right_buf_len, right_buf, p->right_size);
            batch_right_buf_len = batch_right_buf_len + p->right_size;


        }
        if(p->seq % batch_size == 0) {
        
            // fixed size header 32 bytes
            dispatch_to_ds(batch_buf, batch_buf_len, p->id, seq++, p->timestamp, batch_left_buf, batch_left_buf_len, batch_right_buf, batch_right_buf_len);            
            batch_buf = nullptr;
            batch_buf_len =0;
        }
    } else {
        if(batch_buf){
            dispatch_to_ds(batch_buf, batch_buf_len, p->id, seq++, p->timestamp, batch_left_buf, batch_left_buf_len, batch_right_buf, batch_right_buf_len);            
            batch_buf = nullptr;
            batch_buf_len =0;
        }
        dispatch_to_ds(buf, p->size, p->id, seq++, p->timestamp, p->left_buf, p->left_size, p->right_buf, p->right_size);            
    } 
}

char* dispatcher::concat(char* a, size_t a_size, char* b, size_t b_size) {
    char* c = (char*)realloc(a, a_size + b_size);
    memcpy(c + a_size, b,  b_size);  // dest is after "a" data, source is b with b_size
    delete[] b;
    return c;
}



void dispatcher::write_to_ds(int fd, char * buf, int size) {
    if (sendto(fd, buf, size, 0, (struct sockaddr *)&remote, sizeof(remote)) == -1) {
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


