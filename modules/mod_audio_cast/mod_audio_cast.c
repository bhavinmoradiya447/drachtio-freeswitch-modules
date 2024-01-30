#include "mod_audio_cast.h"
#include <uuid/uuid.h>

static unsigned int idxCallCount = 0;

/* Prototypes */
SWITCH_MODULE_SHUTDOWN_FUNCTION(mod_audio_cast_shutdown);
SWITCH_MODULE_RUNTIME_FUNCTION(mod_audio_cast_runtime);
SWITCH_MODULE_LOAD_FUNCTION(mod_audio_cast_load);

/* SWITCH_MODULE_DEFINITION(name, load, shutdown, runtime)
 * Defines a switch_loadable_module_function_table_t and a static const char[] modname
 */
SWITCH_MODULE_DEFINITION(mod_audio_cast, mod_audio_cast_load, mod_audio_cast_shutdown, NULL);

static switch_bool_t capture_callback(switch_media_bug_t *bug, void *user_data, switch_abc_type_t type)
{
	switch_core_session_t *session = switch_core_media_bug_get_session(bug);
	switch (type) {
	case SWITCH_ABC_TYPE_INIT:
	{
		private_t* tech_pvt = (private_t *)  switch_core_media_bug_get_user_data(bug);
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "Got SWITCH_ABC_TYPE_INITl̥ for %s\n", tech_pvt->sessionId);
	}
	break;
	case SWITCH_ABC_TYPE_CLOSE:
		{
        private_t* tech_pvt = (private_t *)  switch_core_media_bug_get_user_data(bug);
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "Got SWITCH_ABC_TYPE_CLOSE for %s\n", tech_pvt->sessionId);
        // send final packet and close
		fork_session_cleanup(session, tech_pvt->bugname, NULL, 1);
		}
		break;
	
	case SWITCH_ABC_TYPE_READ:
		return fork_frame(session, bug);
		
	case SWITCH_ABC_TYPE_WRITE:
	default:
		break;
	}

	return SWITCH_TRUE;
}

  switch_bool_t fork_frame(switch_core_session_t *session, switch_media_bug_t *bug) {
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
            uuid_copy(p->id, tech_pvt->sessionId);
            p->seq = tech_pvt->seq++;
            p->timestamp = chrono::duration_cast<chrono::milliseconds>(chrono::system_clock::now().time_since_epoch()).count();
            p->size = frame.datalen;
l̥           p->buf = frame.data;
            cout << "[trace] " << id << " : " << p->seq << " seq, size: " << p->size << endl;
            tech_pvt->disp->dispatch(p);
        }
	  }
      
    switch_mutex_unlock(tech_pvt->mutex);
    
    return SWITCH_TRUE;
  }

static switch_status_t do_pauseresume(switch_core_session_t *session, char* bugname, int pause)
{
	switch_status_t status = SWITCH_STATUS_SUCCESS;

	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "mod_audio_cast (%s): %s\n", bugname, pause ? "pause" : "resume");
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "fork_session_pauseresume failed because no bug\n");
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
  
    if (!tech_pvt) return SWITCH_STATUS_FALSE;

    switch_core_media_bug_flush(bug);
    tech_pvt->audio_paused = pause;
    return SWITCH_STATUS_SUCCESS;
}

static switch_status_t do_maskunmask(switch_core_session_t *session, char* bugname, int mask)
{
	switch_status_t status = SWITCH_STATUS_SUCCESS;
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "mod_audio_cast (%s): %s\n", bugname, pause ? "pause" : "resume");
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "fork_session_maskunmask failed because no bug\n");
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
  
    if (!tech_pvt) return SWITCH_STATUS_FALSE;

    switch_core_media_bug_flush(bug);
    tech_pvt->audio_masked = mask;
    return SWITCH_STATUS_SUCCESS;
}

static switch_status_t do_stop(switch_core_session_t *session, char* bugname)
{
	switch_status_t status = SWITCH_STATUS_SUCCESS;
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "mod_audio_cast (%s): stop\n", bugname);
	status = fork_session_cleanup(session, bugname, 0);

	return status;
}

  switch_status_t fork_session_cleanup(switch_core_session_t *session, char *bugname, int channelIsClosing) {
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "fork_session_cleanup: no bug %s - already closed\n", bugname);
      return SWITCH_STATUS_FALSE;
    }
    private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
    uint32_t id = tech_pvt->id;

    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) fork_session_cleanup\n", id);

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
      }
    }

    switch_mutex_unlock(tech_pvt->mutex);

    destroy_tech_pvt(tech_pvt);
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "(%u) fork_session_cleanup: connection closed\n", id);
    return SWITCH_STATUS_SUCCESS;
  }


static switch_status_t start_capture(switch_core_session_t *session, 
        switch_media_bug_flag_t flags, 
        int desiredSampling,
        char* bugname)
{
	switch_channel_t *channel = switch_core_session_get_channel(session);
	switch_media_bug_t *bug;
	switch_status_t status;
	switch_codec_t* read_codec;

	void *pUserData = NULL;
    int channels = (flags & SMBF_STEREO) ? 2 : 1;
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, 
    "mod_audio_cast (%s): streaming %d sampling.\n", 
    bugname, desiredSampling);

	if (switch_channel_get_private(channel, bugname)) {
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "mod_audio_cast: bug %s already attached!\n", bugname);
		return SWITCH_STATUS_TRUE;
	}

	read_codec = switch_core_session_get_read_codec(session);

    uint32_t sampling = read_codec->implementation->actual_samples_per_second

	if (switch_channel_pre_answer(channel) != SWITCH_STATUS_SUCCESS) {
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "mod_audio_cast: channel must have reached pre-answer status before calling start!\n");
		return SWITCH_STATUS_FALSE;
	}

    private_t* tech_pvt = (private_t *) switch_core_session_alloc(session, sizeof(private_t));
    if (!tech_pvt) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "error allocating memory!\n");
      return SWITCH_STATUS_FALSE;
    }


     memset(tech_pvt, 0, sizeof(private_t));
  
    strncpy(tech_pvt->sessionId, switch_core_session_get_uuid(session), MAX_SESSION_ID);
    tech_pvt->sampling = desiredSampling;
    tech_pvt->channels = channels;
    tech_pvt->id = ++idxCallCount;
    tech_pvt->audio_paused = 0;
	tech_pvt->seq=0;
	tech_pvt->d = dispatcher::get_instance();
	tech_pvt->audio_masked = 0;
	tech_pvt->graceful_shutdown = 0;
    strncpy(tech_pvt->bugname, bugname, MAX_BUG_LEN);
    
    switch_mutex_init(&tech_pvt->mutex, SWITCH_MUTEX_NESTED, switch_core_session_get_pool(session));

    if (desiredSampling != sampling) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) resampling from %u to %u\n", tech_pvt->id, sampling, desiredSampling);
      tech_pvt->resampler = speex_resampler_init(channels, sampling, desiredSampling, SWITCH_RESAMPLE_QUALITY, &err);
      if (0 != err) {
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error initializing resampler: %s.\n", speex_resampler_strerror(err));
        destroy_tech_pvt(tech_pvt);
		return SWITCH_STATUS_FALSE;
      }
    }
    else {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) no resampling needed for this call\n", tech_pvt->id);
    }

    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "(%u) fork_data_init\n", tech_pvt->id);

    *pUserData = tech_pvt;

	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "adding bug %s.\n", bugname);
	if ((status = switch_core_media_bug_add(session, bugname, NULL, capture_callback, pUserData, 0, flags, &bug)) != SWITCH_STATUS_SUCCESS) {
		return status;
	}
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "setting bug private data %s.\n", bugname);
	switch_channel_set_private(channel, bugname, bug);

	/*if (fork_session_connect(&pUserData) != SWITCH_STATUS_SUCCESS) {
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error mod_audio_cast session cannot connect.\n");
		return SWITCH_STATUS_FALSE;
	} */
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "exiting start_capture.\n");
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

#define CAST_API_SYNTAX "<uuid> [start | stop ]"
SWITCH_STANDARD_API(cast_function)
{
	char *mycmd = NULL, *argv[2] = { 0 };
	int argc = 0;
	switch_status_t status = SWITCH_STATUS_FALSE;
  char *bugname = MY_BUG_NAME;

	if (!zstr(cmd) && (mycmd = strdup(cmd))) {
		argc = switch_separate_string(mycmd, ' ', argv, (sizeof(argv) / sizeof(argv[0])));
	}
	assert(cmd);
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "mod_audio_cast cmd: %s\n", cmd);


	if (zstr(cmd) || argc < 2) {

		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error with command %s %s %s.\n", cmd, argv[0], argv[1]);
		stream->write_function(stream, "-USAGE: %s\n", CAST_API_SYNTAX);
		goto done;
	} else {
		switch_core_session_t *lsession = NULL;

		if ((lsession = switch_core_session_locate(argv[0]))) {
			if (!strcasecmp(argv[1], "stop")) {
        		status = do_stop(lsession, bugname);
      		}
			else if (!strcasecmp(argv[1], "pause")) {
				status = do_pauseresume(lsession, bugname, 1);
			}
			else if (!strcasecmp(argv[1], "resume")) {
				status = do_pauseresume(lsession, bugname, 0);
			}
			else if (!strcasecmp(argv[1], "mast")) {
				status = do_maskunmask(lsession, bugname, 1);
			}
			else if (!strcasecmp(argv[1], "unmask")) {
				status = do_maskunmask(lsession, bugname, 0);
			}
			else if (!strcasecmp(argv[1], "start")) {
				switch_channel_t *channel = switch_core_session_get_channel(lsession);
				int sampling = 8000;
				switch_media_bug_flag_t flags = SMBF_READ_STREAM ;
				flags |= SMBF_WRITE_STREAM ;
				flags |= SMBF_STEREO;
				status = start_capture(lsession, flags, sampling, bugname);
			}
			else {
				switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "unsupported mod_audio_cast cmd: %s\n", argv[1]);
			}
			switch_core_session_rwunlock(lsession);
		}
		else {
			switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error locating session %s\n", argv[0]);
		}
	}

	if (status == SWITCH_STATUS_SUCCESS) {
		stream->write_function(stream, "+OK Success\n");
	} else {
		stream->write_function(stream, "-ERR Operation Failed\n");
	}

  done:

	switch_safe_free(mycmd);
	return SWITCH_STATUS_SUCCESS;
}


SWITCH_MODULE_LOAD_FUNCTION(mod_audio_cast_load)
{
	switch_api_interface_t *api_interface;

	switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_NOTICE, "mod_audio_cast API loading..\n");

	/* connect my internal structure to the blank pointer passed to me */
	*module_interface = switch_loadable_module_create_module_interface(pool, modname);


	SWITCH_ADD_API(api_interface, "mod_audio_cast", "audio_cast API", cast_function, CAST_API_SYNTAX);
	switch_console_set_complete("add mod_audio_cast start");
	switch_console_set_complete("add mod_audio_cast stop");
	dispatcher::get_instance();

	switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_NOTICE, "mod_audio_cast API successfully loaded\n");

	/* indicate that the module should continue to be loaded */
  //mod_running = 1;


	return SWITCH_STATUS_SUCCESS;
}
/*
  Called when the system shuts down
  Macro expands to: switch_status_t mod_audio_cast_shutdown() */
SWITCH_MODULE_SHUTDOWN_FUNCTION(mod_audio_cast_shutdown)
{
	dispatcher::get_instance()->stop();
	return SWITCH_STATUS_SUCCESS;
}
