#include "dispatcher.h"
#include <errno.h>


dispatcher::dispatcher() {
    std::fill_n(ready_arr, POOL_SIZE, false);
    std::fill_n(processed_arr, POOL_SIZE, false);
    std::fill_n(done_arr, POOL_SIZE, false);
   // fd = open(myfifo, O_WRONLY);
   consistance_hash = new consistence_hashing(1000);
   for(int i=0; i< POOL_SIZE; i++){
    consistance_hash->addNode(std::to_string(i));    
    char * file_path = switch_mprintf("%s-%d", fifo_file_prefix, i); 
    mkfifo(file_path, 0666);
    fd_arr[i] = open(file_path, O_WRONLY);
   }
}

dispatcher::~dispatcher() {
    for(int i=0; i< POOL_SIZE; i++){
        char * file_path = switch_mprintf("%s-%d", fifo_file_prefix, i); 
        close(fd_arr[i]);
        unlink(file_path);
    }
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

void dispatcher::dispatch(payload * p, char * uuid) {

    int index = stoi(consistance_hash->getNode(uuid));
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
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] queued end of stream: %s\n", uuid);
    }
    unique_lock<mutex> lck(mtx_arr[index]);
    q_arr[index].push_back(buf);
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] Queue %d size is : %d\n", index, q_arr[index].size());
    ready_arr[index] = true;
    cv_arr[index].notify_one();
}

void dispatcher::run(int index) {
    while (true) {
        unique_lock<mutex> lck(mtx_arr[index]);
        // cout << "dispatcher waiting to read" << endl;
        if (q_arr[index].empty()) {
            cv_arr[index].wait(lck, [this, index]{return ready_arr[index] || done_arr[index];});
        }   
        // cv.wait(lck, [this]{return ready || done;});
        if(done_arr[index]) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO,"[info] dispatcher done for %d\n", index);
            close(fd_arr[index]);
            break;
        }
        // cout << "dispatcher read" << endl;
        char * buf = q_arr[index].front();
        q_arr[index].pop_front();
        ready_arr[index] = false;
        lck.unlock();
        // read size from buf
        int size;
        int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
        int size_pos = 16 + sizeof(int) + sizeof(long);
        memcpy(&size, buf + size_pos, sizeof(int));
        int ret = write(fd_arr[index], buf, header_size + size);
        if (ret < 0)
        {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Error writing to pipe %d: %s\n", index, strerror(errno));
            goto end;
        } else if (ret < header_size + size) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Partial Write happend on named pipe %d, expteded %d but wrote only %d\n", index, (header_size + size), ret);
        }
        else 
        {
            //cout << "[info] sent end of stream" << endl;
            // flush
            fsync(fd_arr[index]);
        }
        end:
        delete[] buf;
        processed_arr[index] = true;
        
    }
}

void dispatcher::stop() {
    for(int index =0; index< POOL_SIZE; index ++) {
        unique_lock<mutex> lck(mtx_arr[index]);
        done_arr[index] = true;
        cv_arr[index].notify_all();
    }
}