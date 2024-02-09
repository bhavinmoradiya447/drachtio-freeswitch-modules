#include "dispatcher.h"

dispatcher::dispatcher() {
    mkfifo(myfifo, 0666);
   // fd = open(myfifo, O_WRONLY);
}

dispatcher::~dispatcher() {
   // close(fd);
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

void dispatcher::dispatch(char * buf) {
    
    lock_guard<mutex> lck(mtx);
    fd = open(myfifo, O_WRONLY | O_NONBLOCK);
    if(fd < 0) {
        if(q.size() > QUEUE_MAX_SIZE) {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR,"[ERROR] queued is fulled, ignoring audio stream\n");
            delete[] buf;
            return;
        } else {
            switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_DEBUG,"[ERROR] Pushing to Queue\n");
            q.push_back(buf);
        }
    } else {
        write_to_file(fd, buf);
        while(!q.empty()){
            buf = q.front();
            q.pop_front();
            write_to_file(fd, buf);
        }
        close(fd);
    }
  
}

void dispatcher::write_to_file(int fd, char * buf) {
    int size;
    int header_size = 16 + sizeof(int) + sizeof(long) + sizeof(int);
    int size_pos = 16 + sizeof(int) + sizeof(long);
    memcpy(&size, buf + size_pos, sizeof(int));
    int ret = write(fd, buf, header_size + size);
    if (ret < 0)
    {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "Error writing to pipe: %d\n", ret);
        return;
    }
    else {
        fsync(fd);
    }
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
    unlink(myfifo);
}