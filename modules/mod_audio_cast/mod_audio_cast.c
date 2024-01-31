#include "mod_audio_cast.h"
#include "audio_cast_helper.h"


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
		audio_cast_session_cleanup(session, tech_pvt->bugname, 1);
	}
	break;
	
	case SWITCH_ABC_TYPE_READ:
		return audio_cast_frame(session, bug);
		
	case SWITCH_ABC_TYPE_WRITE:
	default:
		break;
	}

	return SWITCH_TRUE;
}



static switch_status_t do_pauseresume(switch_core_session_t *session, char* bugname, int pause)
{
	switch_status_t status = SWITCH_STATUS_SUCCESS;

	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "mod_audio_stream (%s): %s\n", bugname, pause ? "pause" : "resume");
	status = audio_cast_session_pauseresume(session, bugname, pause);

	return status;
}

static switch_status_t do_maskunmask(switch_core_session_t *session, char* bugname, int mask)
{
	switch_status_t status = SWITCH_STATUS_SUCCESS;

	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "mod_audio_stream (%s): %s\n", bugname, mask ? "mask" : "unmask");
	status = audio_cast_session_maskunmask(session, bugname, mask);
	return status;
}

static switch_status_t do_stop(switch_core_session_t *session, char* bugname)
{
	switch_status_t status = SWITCH_STATUS_SUCCESS;
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "mod_audio_cast (%s): stop\n", bugname);
	status = audio_cast_session_cleanup(session, bugname, 0);

	return status;
}

static switch_status_t start_capture(switch_core_session_t *session, 
        switch_media_bug_flag_t flags, 
        int sampling,
        char* bugname)
{
	switch_channel_t *channel = switch_core_session_get_channel(session);
	switch_media_bug_t *bug;
	switch_status_t status;
	switch_codec_t* read_codec;

	void *pUserData = NULL;
    int channels = (flags & SMBF_STEREO) ? 2 : 1;
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, 
    "mod_audio_stream (%s): streaming %d sampling.\n", 
    bugname, sampling);

	if (switch_channel_get_private(channel, bugname)) {
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "mod_audio_cast: bug %s already attached!\n", bugname);
		return SWITCH_STATUS_FALSE;
	}

	read_codec = switch_core_session_get_read_codec(session);

	if (switch_channel_pre_answer(channel) != SWITCH_STATUS_SUCCESS) {
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "mod_audio_cast: channel must have reached pre-answer status before calling start!\n");
		return SWITCH_STATUS_FALSE;
	}

	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "calling audio_cast_session_init.\n");
	if (SWITCH_STATUS_FALSE == audio_cast_session_init(session, read_codec->implementation->actual_samples_per_second, 
		sampling, channels, bugname, &pUserData)) {
		switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error initializing mod_audio_cast session.\n");
		return SWITCH_STATUS_FALSE;
	}
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "adding bug %s.\n", bugname);
	if ((status = switch_core_media_bug_add(session, bugname, NULL, capture_callback, pUserData, 0, flags, &bug)) != SWITCH_STATUS_SUCCESS) {
		return status;
	}
	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "setting bug private data %s.\n", bugname);
	switch_channel_set_private(channel, bugname, bug);

	switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "exiting start_capture.\n");
	return SWITCH_STATUS_SUCCESS;
}


#define CAST_API_SYNTAX "<uuid> [start | stop ]"
SWITCH_STANDARD_API(cast_function)
{
	char *mycmd = NULL, *argv[2] = { 0 };
	int argc = 0;
	switch_status_t status = SWITCH_STATUS_FALSE;
    char *bugname = "audio_cast";

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
	audio_cast_init();

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
	audio_cast_cleanup();
	return SWITCH_STATUS_SUCCESS;
}
