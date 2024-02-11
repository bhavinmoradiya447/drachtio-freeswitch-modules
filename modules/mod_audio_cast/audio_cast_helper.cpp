#include "mod_audio_cast.h"
#include "dispatcher.h"
#include <switch_curl.h>

namespace {
  static unsigned int idxCallCount = 0;
  
  switch_status_t audio_cast_data_init(private_t *tech_pvt, switch_core_session_t *session, int sampling, int desiredSampling, int channels, 
    char *bugname) {

    int err;
    switch_codec_implementation_t read_impl;
    switch_channel_t *channel = switch_core_session_get_channel(session);

    switch_core_session_get_read_impl(session, &read_impl);
  
    memset(tech_pvt, 0, sizeof(private_t));
  
   strncpy(tech_pvt->sessionId, switch_core_session_get_uuid(session), MAX_SESSION_ID);
    tech_pvt->sampling = desiredSampling;
    tech_pvt->channels = channels;
    tech_pvt->id = ++idxCallCount;
    tech_pvt->audio_paused = 0;
	  tech_pvt->seq=0;
	  tech_pvt->disp = static_cast<void *>(dispatcher::get_instance());
	  tech_pvt->audio_masked = 0;
	  tech_pvt->graceful_shutdown = 0;
    strncpy(tech_pvt->bugname, bugname, MAX_BUG_LEN);
    strncpy(tech_pvt->mcsurl, "http://localhost:3030", MAX_URL_LEN);

   switch_mutex_init(&tech_pvt->mutex, SWITCH_MUTEX_NESTED, switch_core_session_get_pool(session));

    if (desiredSampling != sampling) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) resampling from %u to %u\n", tech_pvt->id, sampling, desiredSampling);
      tech_pvt->resampler = speex_resampler_init(channels, sampling, desiredSampling, SWITCH_RESAMPLE_QUALITY, &err);
      if (0 != err) {
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error initializing resampler: %s.\n", speex_resampler_strerror(err));
        return SWITCH_STATUS_FALSE;
      }
    }
    else {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) no resampling needed for this call\n", tech_pvt->id);
    }

    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) audio_cast_data_init\n", tech_pvt->id);

    return SWITCH_STATUS_SUCCESS;
  }

  void destroy_tech_pvt(private_t* tech_pvt) {
    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "%s (%u) destroy_tech_pvt\n", tech_pvt->sessionId, tech_pvt->id);
    if (tech_pvt->resampler) {
      speex_resampler_destroy(tech_pvt->resampler);
      tech_pvt->resampler = nullptr;
    }
    if (tech_pvt->mutex) {
      switch_mutex_destroy(tech_pvt->mutex);
      tech_pvt->mutex = nullptr;
    }
  }
}

extern "C" {

  switch_status_t audio_cast_init() {
   //dispatcher::get_instance();
   return SWITCH_STATUS_SUCCESS;
  }

  switch_status_t audio_cast_cleanup() {
    dispatcher::get_instance()->stop();
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

  switch_status_t audio_cast_call_mcs(switch_core_session_t *session, char* hostName) {
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
	

		curl_json_text = switch_mprintf("{\"uuid\": \"%s\", \"address\": \"%s\"}", uuid, hostName);
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

			destUrl = switch_mprintf("%s","http://localhost:3030/start_cast");
			switch_curl_easy_setopt(curl_handle, CURLOPT_URL, destUrl);

			switch_curl_easy_perform(curl_handle);
			switch_curl_easy_getinfo(curl_handle, CURLINFO_RESPONSE_CODE, &httpRes);
			switch_safe_free(destUrl);
			if (httpRes >= 200 && httpRes < 300) {
				goto end;
			} else {
				switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Got error [%ld] posting to web server [%s]\n",
								  httpRes, "http://localhost:3030/start_cast");
				
				switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Retry will be with url [%s]\n", "http://localhost:3030/start_cast");
				
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
    if (SWITCH_STATUS_SUCCESS != audio_cast_data_init(tech_pvt, session, samples_per_second, sampling, channels, 
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
    //AudioPipe *pAudioPipe = static_cast<AudioPipe *>(tech_pvt->pAudioPipe);
      
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
        disp->dispatch(p, tech_pvt->sessionId);
        delete p;
        
      }
    }

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

  switch_status_t do_http_2_mcs(const char* url, const char * contentType, const char * data) {
    switch_CURL *curl_handle = NULL;
    switch_curl_slist_t *headers = NULL;
    curl_handle = switch_curl_easy_init();
    char *ct = switch_mprintf("Content-Type: %s", contentType);
    headers = switch_curl_slist_append(headers, ct);
    switch_safe_free(ct);
    switch_curl_easy_setopt(curl_handle, CURLOPT_URL, url);
    switch_curl_easy_setopt(curl_handle, CURLOPT_POSTFIELDSIZE, strlen(data));
    switch_curl_easy_setopt(curl_handle, CURLOPT_POSTFIELDS, (void *) data);
    switch_curl_easy_setopt(curl_handle, CURLOPT_USERAGENT, "freeswitch-json/1.0");
    if (headers) {
        switch_curl_easy_setopt(curl_handle, CURLOPT_HTTPHEADER, headers);
    }

    switch_curl_easy_perform(curl_handle);
    long httpRes = 0;
    switch_curl_easy_getinfo(curl_handle, CURLINFO_RESPONSE_CODE, &httpRes);
    switch_curl_easy_cleanup(curl_handle);
    switch_curl_slist_free_all(headers);
    if (httpRes == 200) {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "do_http_2_mcs (%s) SUCCESS\n", data);
        return SWITCH_STATUS_SUCCESS;
    } else {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_INFO, "do_http_2_mcs (%s) FAIL\n", data);
        return SWITCH_STATUS_FALSE;
    }
  }

  switch_status_t send_dtmf(char * url, char *text) {
    if (!text) {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "senddtmf NULL\n");
        return SWITCH_STATUS_FALSE;
    }
    if (std::string("0123456789*#").find(*text) == std::string::npos) {
        switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "senddtmf (%s) INVALID\n", text);
        return SWITCH_STATUS_FALSE;
    }
    const char * contentType = "application/json";
    char *data = switch_mprintf("{\"dtmf\": \"%s\"}", text);
    switch_status_t status =  do_http_2_mcs(url, contentType, (const char *)data);
    switch_safe_free(data);
    return status;
  }

  switch_status_t audio_cast_session_sendtext(switch_core_session_t *session, char *bugname, char *text) {
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "audio_cast_session_sendtext failed because no bug\n");
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);

    if (!tech_pvt) return SWITCH_STATUS_FALSE;

    return send_dtmf(tech_pvt->mcsurl, text);
  }

  switch_bool_t audio_cast_frame(switch_core_session_t *session, switch_media_bug_t *bug) {
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);

    if (!tech_pvt || tech_pvt->audio_paused || tech_pvt->graceful_shutdown) return SWITCH_TRUE;
    
	// TODO:: Add logic for mask 
    // TODO:: Add logic for resample 
    if (switch_mutex_trylock(tech_pvt->mutex) == SWITCH_STATUS_SUCCESS) {
        //private_t* tech_pvt = (private_t *)  switch_core_media_bug_get_user_data(bug);
        uint8_t data[SWITCH_RECOMMENDED_BUFFER_SIZE];
        switch_frame_t frame = { 0 };
        frame.data = data;
        frame.buflen = SWITCH_RECOMMENDED_BUFFER_SIZE;
        
        while (switch_core_media_bug_read(bug, &frame, SWITCH_TRUE) == SWITCH_STATUS_SUCCESS) {
//           write(fd, frame.data , frame.datalen);
          payload * p = new payload;
          uuid_parse(tech_pvt->sessionId, p->id);
          p->seq = tech_pvt->seq++;
          p->timestamp = chrono::duration_cast<chrono::milliseconds>(chrono::system_clock::now().time_since_epoch()).count();
          p->size = frame.datalen;
          p->buf =  static_cast<char *>(frame.data);
          dispatcher *disp = static_cast<dispatcher *>(tech_pvt->disp);
          disp->dispatch(p, tech_pvt->sessionId);
          delete p;
        }
	  }
      
    switch_mutex_unlock(tech_pvt->mutex);
    
    return SWITCH_TRUE;
  
  }

}

