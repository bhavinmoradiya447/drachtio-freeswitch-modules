#include "mod_audio_cast.h"
#include "dispatcher.h"
#include <switch_curl.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include <g711.h>


namespace {
  static unsigned int idxCallCount = 0;
  
  switch_status_t audio_cast_data_init(private_t *tech_pvt, switch_core_session_t *session, responseHandler_t responseHandler, int sampling, int desiredSampling, int channels, 
    char *bugname) {

    int err;
    switch_codec_implementation_t read_impl;
    switch_channel_t *channel = switch_core_session_get_channel(session);

    switch_core_session_get_read_impl(session, &read_impl);
  
    memset(tech_pvt, 0, sizeof(private_t));
  
   dispatcher * disp = new dispatcher(switch_core_session_get_uuid(session));
   if(!disp->connet_ds_socket()){
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Failed to open Domain Socket for %s\n", switch_core_session_get_uuid(session));
    return SWITCH_STATUS_FALSE;
   }

   strncpy(tech_pvt->sessionId, switch_core_session_get_uuid(session), MAX_SESSION_ID);
    tech_pvt->sampling = desiredSampling;
    tech_pvt->channels = channels;
    tech_pvt->id = ++idxCallCount;
    tech_pvt->audio_paused = 0;
    tech_pvt->seq=1;
    tech_pvt->disp = static_cast<void *>(disp);
    tech_pvt->audio_masked = 0;
    tech_pvt->graceful_shutdown = 0;
    tech_pvt->responseHandler = responseHandler;
    switch_core_hash_init_case(&tech_pvt->client_address_hash, SWITCH_FALSE);
    strncpy(tech_pvt->bugname, bugname, MAX_BUG_LEN);
    

   switch_mutex_init(&tech_pvt->mutex, SWITCH_MUTEX_NESTED, switch_core_session_get_pool(session));
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) audio_cast_data_init\n", tech_pvt->id);

    return SWITCH_STATUS_SUCCESS;
  }

  void destroy_tech_pvt(private_t* tech_pvt) {
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s (%u) destroy_tech_pvt\n", tech_pvt->sessionId, tech_pvt->id);
    if (tech_pvt->read_resampler) {
      switch_resample_destroy(&tech_pvt->read_resampler);
      tech_pvt->read_resampler = nullptr;
    }
    if (tech_pvt->mutex) {
      switch_mutex_destroy(tech_pvt->mutex);
      tech_pvt->mutex = nullptr;
    }
    if (tech_pvt->disp) {
      tech_pvt->disp = nullptr;
    }

    switch_core_hash_destroy(&tech_pvt->client_address_hash);
  }
}

extern "C" {

  switch_status_t audio_cast_init() {
   return SWITCH_STATUS_SUCCESS;
  }

  switch_status_t audio_cast_cleanup() {
    return SWITCH_STATUS_SUCCESS;
  }

  /* this function would have access to the HTML returned by the webserver, we don't need it
  * and the default curl activity is to print to stdout, something not as desirable
  * so we have a dummy function here
  */
  size_t httpCallBack(char *buffer, size_t size, size_t nitems, void *outstream)
  {
    return size * nitems;
  }

  switch_status_t audio_cast_call_mcs(switch_core_session_t *session, char* payload, char* url) {
    char* uuid = switch_core_session_get_uuid(session);
    char *curl_json_text = NULL;
    long httpRes;
    CURL *curl_handle = NULL;
    switch_curl_slist_t *headers = NULL;
    switch_curl_slist_t *slist = NULL;
    int fd = -1;
    uint32_t cur_try;


    char *destUrl = NULL;
    curl_handle = switch_curl_easy_init();
    headers = switch_curl_slist_append(headers, "Content-Type: application/json");
  

    curl_json_text = switch_mprintf("%s", payload);
    switch_assert(curl_json_text != NULL);

    
    switch_curl_easy_setopt(curl_handle, CURLOPT_HTTPHEADER, headers);
    switch_curl_easy_setopt(curl_handle, CURLOPT_POST, 1);
    switch_curl_easy_setopt(curl_handle, CURLOPT_NOSIGNAL, 1);
    switch_curl_easy_setopt(curl_handle, CURLOPT_POSTFIELDS, curl_json_text);
    switch_curl_easy_setopt(curl_handle, CURLOPT_USERAGENT, "freeswitch-json/1.0");
    switch_curl_easy_setopt(curl_handle, CURLOPT_WRITEFUNCTION, httpCallBack);


    // tcp timeout
    switch_curl_easy_setopt(curl_handle, CURLOPT_TIMEOUT, 3);

    /* these were used for testing, optionally they may be enabled if someone desires
       switch_curl_easy_setopt(curl_handle, CURLOPT_FOLLOWLOCATION, 1); // 302 recursion level
     */

    for (cur_try = 0; cur_try < 3; cur_try++) {
      if (cur_try > 0) {
        switch_yield(200000);
      }

      destUrl = switch_mprintf("%s", url);
      switch_curl_easy_setopt(curl_handle, CURLOPT_URL, destUrl);

      switch_curl_easy_perform(curl_handle);
      switch_curl_easy_getinfo(curl_handle, CURLINFO_RESPONSE_CODE, &httpRes);
      switch_safe_free(destUrl);
      if (httpRes >= 200 && httpRes < 300) {
        goto end;
      } else {
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Got error [%ld] posting to web server [%s]\n",
                  httpRes, url);
        
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Retry will be with url [%s]\n", url);
        
      }
    }
    
    switch_curl_easy_cleanup(curl_handle);
    switch_curl_slist_free_all(headers);
    switch_curl_slist_free_all(slist);
    slist = NULL;
    headers = NULL;
    curl_handle = NULL;

    /* if we are here the web post failed for some reason */
    
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Unable to post to mcs server\n");
    return SWITCH_STATUS_FALSE;

    end:
    if (curl_handle) {
      switch_curl_easy_cleanup(curl_handle);
    }
    if (headers) {
      switch_curl_slist_free_all(headers);
    }
    if (slist) {
      switch_curl_slist_free_all(slist);
    }
    switch_safe_free(curl_json_text);
    return SWITCH_STATUS_SUCCESS;
  }

  switch_status_t audio_cast_session_init(switch_core_session_t *session, 
              responseHandler_t responseHandler,
              uint32_t samples_per_second, 
              int sampling,
              int channels,
              char *bugname,
              void **ppUserData)
  {    	
    int err;

    // allocate per-session data structure
    private_t* tech_pvt = (private_t *) switch_core_session_alloc(session, sizeof(private_t));
    if (!tech_pvt) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "error allocating memory!\n");
      return SWITCH_STATUS_FALSE;
    }
    if (SWITCH_STATUS_SUCCESS != audio_cast_data_init(tech_pvt, session, responseHandler, samples_per_second, sampling, channels, 
      bugname)) {
      destroy_tech_pvt(tech_pvt);
      return SWITCH_STATUS_FALSE;
    }

    *ppUserData = tech_pvt;
    return SWITCH_STATUS_SUCCESS;
  }

switch_status_t audio_cast_session_cleanup(switch_core_session_t *session, char *bugname, int channelIsClosing) {
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "audio_cast_session_cleanup: no bug %s - websocket conection already closed\n", bugname);
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
    uint32_t id = tech_pvt->id;

    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) audio_cast_session_cleanup\n", id);

    if (!tech_pvt) return SWITCH_STATUS_FALSE;
      
    switch_mutex_lock(tech_pvt->mutex);

    // get the bug again, now that we are under lock
    {
      switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
      if (bug) {
        switch_channel_set_private(channel, bugname, NULL);
        if (!channelIsClosing) {
          switch_core_media_bug_remove(session, &bug);
        } 

        payload * p = new payload;
        uuid_parse(tech_pvt->sessionId, p->id);
        p->seq = tech_pvt->seq++;
        p->timestamp = chrono::duration_cast<chrono::milliseconds>(chrono::system_clock::now().time_since_epoch()).count();
        p->size = 0;
        p->buf =  NULL;
        dispatcher *disp = static_cast<dispatcher *>(tech_pvt->disp);
        disp->dispatch(p);
        disp->stop();
        delete p; 
      }
    }
    tech_pvt->responseHandler(session, EVENT_CAST_CLOSE, tech_pvt->seq, NULL, NULL, NULL);
    destroy_tech_pvt(tech_pvt);
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "(%u) audio_cast_session_cleanup: connection closed\n", id);
    return SWITCH_STATUS_SUCCESS;
  }

switch_status_t audio_cast_session_maskunmask(switch_core_session_t *session, char *bugname, int mask){
  switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "audio_cast_session_mastunmask failed because no bug\n");
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
  
    if (!tech_pvt) return SWITCH_STATUS_FALSE;

    switch_core_media_bug_flush(bug);
    tech_pvt->audio_masked = mask;
    if(mask) {
        tech_pvt->responseHandler(session, EVENT_CAST_MASK, tech_pvt->seq, NULL, NULL, NULL);
    } else {
      tech_pvt->responseHandler(session, EVENT_CAST_UNMASK, tech_pvt->seq, NULL, NULL, NULL);
    }
    return SWITCH_STATUS_SUCCESS;
}

  switch_status_t audio_cast_session_pauseresume(switch_core_session_t *session, char *bugname, int pause) {
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "audio_cast_session_pauseresume failed because no bug\n");
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
  
    if (!tech_pvt) return SWITCH_STATUS_FALSE;

    switch_core_media_bug_flush(bug);
    tech_pvt->audio_paused = pause;
    if(pause) {
        tech_pvt->responseHandler(session, EVENT_CAST_PAUSE, tech_pvt->seq, NULL, NULL, NULL);
    } else {
      tech_pvt->responseHandler(session, EVENT_CAST_RESUME, tech_pvt->seq, NULL, NULL, NULL);
    }
    return SWITCH_STATUS_SUCCESS;
  }

  switch_status_t audio_cast_session_graceful_shutdown(switch_core_session_t *session, char *bugname) {
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "audio_cast_session_graceful_shutdown failed because no bug\n");
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
  
    if (!tech_pvt) return SWITCH_STATUS_FALSE;

    tech_pvt->graceful_shutdown = 1;

    return SWITCH_STATUS_SUCCESS;
  }

  switch_bool_t convert_linear2_g711_pcmu8k(char* frame, uint32_t* framelen) {
      short *inbuf = (short *)frame;
      uint32_t in_data_len = *framelen;
      uint32_t outBufSize = in_data_len / sizeof(short);
      unsigned char* obuf = (unsigned char*)  malloc(outBufSize);
      uint32_t i;

      for (i = 0; i < outBufSize; i++) {
          obuf[i] = linear_to_ulaw(inbuf[i]);
      }
      memcpy(frame, obuf, outBufSize);
      *framelen = i;
      free(obuf);
      return SWITCH_TRUE;
  }

  switch_bool_t audio_cast_frame(switch_core_session_t *session, switch_media_bug_t *bug) {
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);

    if (!tech_pvt || tech_pvt->audio_paused || tech_pvt->graceful_shutdown) return SWITCH_TRUE;
  
    if (switch_mutex_trylock(tech_pvt->mutex) == SWITCH_STATUS_SUCCESS) {
        uint8_t data[SWITCH_RECOMMENDED_BUFFER_SIZE];
        switch_frame_t frame = { 0 };
        frame.data = data;
        frame.buflen = SWITCH_RECOMMENDED_BUFFER_SIZE;
        
        while (switch_core_media_bug_read(bug, &frame, SWITCH_TRUE) == SWITCH_STATUS_SUCCESS) {
          if( tech_pvt->audio_masked ) { 
             unsigned char null_data[SWITCH_RECOMMENDED_BUFFER_SIZE] = {0};
            memcpy(frame.data, null_data, frame.datalen);
          }

          // Resample to 8000
          if(tech_pvt->read_resampler) {
            switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "Resampaling\n");

            int16_t *org_data = (int16_t *) frame.data;
            switch_resample_process(tech_pvt->read_resampler, org_data, (int) frame.datalen / 2);
            uint32_t new_len = tech_pvt->read_resampler->to_len * 2;
            memcpy(frame.data, tech_pvt->read_resampler->to, new_len);
            frame.datalen = new_len;
          }


          convert_linear2_g711_pcmu8k((char *)frame.data, &frame.datalen);
          payload * p = new payload;
          uuid_parse(tech_pvt->sessionId, p->id);
          p->seq = tech_pvt->seq++;
          p->timestamp = chrono::duration_cast<chrono::milliseconds>(chrono::system_clock::now().time_since_epoch()).count();
          p->size = frame.datalen;
          p->buf =  static_cast<char *>(frame.data);
          dispatcher *disp = static_cast<dispatcher *>(tech_pvt->disp);
          disp->dispatch(p);
          delete p;
        }
    }
      
    switch_mutex_unlock(tech_pvt->mutex);
    
    return SWITCH_TRUE;
  
  }


}

