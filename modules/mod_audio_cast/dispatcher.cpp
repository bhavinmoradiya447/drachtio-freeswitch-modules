#include "dispatcher.h"
#include <errno.h>

dispatcher::dispatcher() {
   // fd = open(myfifo, O_WRONLY);
   consistance_hash = new consistence_hashing(1000);
   for(int i=0; i< POOL_SIZE; i++){
    consistance_hash->addNode(std::to_string(i));    
    fifo_files[i] = switch_mprintf("/tmp/mod-audio-cast-pipe-%d", i); 
    mkfifo(fifo_files[i], 0666);
   }
 
}

dispatcher::~dispatcher() {
   // close(fd);
    for(int i=0; i< POOL_SIZE; i++){
        unlink(fifo_files[i]);
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

void dispatcher::dispatch(char * buf, char * uuid) {
    
    int index = stoi(consistance_hash->getNode(uuid));

    lock_guard<mutex> lck(mtx_arr[index]);
    fd = open(fifo_files[index], O_WRONLY | O_NONBLOCK);
    if(fd < 0) {
        //
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] Unable to open named pipe: %s\n", strerror(errno));
        push_to_queue(buf, index);
    } else {
        int status = write_to_file(fd, buf);
        if(status < 0) {
              push_to_queue(buf, index);
        }
        while(!q_arr[index].empty()){
            buf = q_arr[index].front();
            q_arr[index].pop_front();
            int status = write_to_file(fd, buf);
             if(status < 0) {
                push_to_queue(buf, index);
                break;
            }
        }
       
    }
   close(fd);
}

void dispatcher::push_to_queue(char * buf, int index) {
    if(q_arr[index].size() > QUEUE_MAX_SIZE) {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] queue %d is fulled, ignoring audio stream\n", index);
        delete[] buf;
        return;
    } else {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_DEBUG,"[ERROR] Pushing to Queue\n");
        q_arr[index].push_back(buf);
    }
}
int dispatcher::write_to_file(int fd, char * buf) {
    int size;
    int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
    int size_pos = 16 + sizeof(int) + sizeof(long);
    memcpy(&size, buf + size_pos, sizeof(int));
    int ret = write(fd, buf, header_size + size);
    if (ret < 0)
    {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "Error writing to pipe: %d, ERROR: %s\n", ret, strerror(errno));
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
       
        ready = false;
        lck.unlock();
        // read size from buf
        int size;
        int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
        int size_pos = 16 + sizeof(int) + sizeof(long);
        memcpy(&size, buf + size_pos, sizeof(int));
        //fd = open(myfifo, O_WRONLY);
        int ret = write(fd, buf, header_size + size);
        //close(fd);
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
        buf = nullptr;
        processed = true;
    }
}
*/

void dispatcher::stop() {
   // unique_lock<mutex> lck(mtx);
  //  done = true;
   // cv.notify_all();
    //close(fd);
    for(int i=0; i< POOL_SIZE; i++){
        unlink(fifo_files[i]);
    }
}