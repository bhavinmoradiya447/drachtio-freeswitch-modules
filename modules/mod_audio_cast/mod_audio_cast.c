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

static void responseHandler(switch_core_session_t* session, const char* eventName, int last_seq, const char* address, char* payload) {
	switch_event_t *event;
    int duration_ms = 320 * (last_seq -1 ) / 2 / 8; // per packet data byte * number of packets / 2 channel / 8k bit rate

	switch_channel_t *channel = switch_core_session_get_channel(session);
	if (payload) switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "responseHandler: sending event payload: %s.\n", payload);
	switch_event_create_subclass(&event, SWITCH_EVENT_CUSTOM, eventName);
	switch_channel_event_set_data(channel, event);
    if (address) switch_event_add_header_string(event, SWITCH_STACK_BOTTOM, "Client-Address", address);
    switch_event_add_header(event, SWITCH_STACK_BOTTOM, "duration_ms", "%d", duration_ms);
    if (payload) switch_event_add_header_string(event, SWITCH_STACK_BOTTOM, "Payload", payload);

	switch_event_fire(&event);
}

static switch_bool_t capture_callback(switch_media_bug_t *bug, void *user_data, switch_abc_type_t type)
{
    switch_core_session_t *session = switch_core_media_bug_get_session(bug);
    switch (type) {
    case SWITCH_ABC_TYPE_INIT:
    {
        switch_codec_implementation_t read_impl;
        private_t* tech_pvt = (private_t *)  switch_core_media_bug_get_user_data(bug);
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "Got SWITCH_ABC_TYPE_INITl̥ for %s\n", tech_pvt->sessionId);
        switch_core_session_get_read_impl(session, &read_impl);

        if (read_impl.actual_samples_per_second != 8000) {
            switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "Resampling from %d to %d for call-id %s\n", read_impl.actual_samples_per_second, 8000, tech_pvt->sessionId);

            switch_resample_create(&tech_pvt->read_resampler,
                                    read_impl.actual_samples_per_second,
                                    8000,
                                    320, SWITCH_RESAMPLE_QUALITY, 1);
        }

    }
    break;
    case SWITCH_ABC_TYPE_CLOSE:
    {
        private_t* tech_pvt = (private_t *)  switch_core_media_bug_get_user_data(bug);
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "Got SWITCH_ABC_TYPE_CLOSE for %s\n", tech_pvt->sessionId);
        // send final packet and close
        audio_cast_frame(session, bug);
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

static switch_status_t do_stop(switch_core_session_t *session, char* bugname, char* payload)
{
    switch_status_t status = SWITCH_STATUS_SUCCESS;
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    const char* address = cJSON_GetObjectCstr(cJSON_Parse(payload), "address");
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, "mod_audio_cast (%s): stop\n", bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "audio_cast_session_cleanup: no bug %s - websocket conection already closed\n", bugname);
      return SWITCH_STATUS_FALSE;
    }
    {
        private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);

        if (switch_core_hash_delete(tech_pvt->client_address_hash, address) != NULL 
        && SWITCH_STATUS_FALSE == audio_cast_call_mcs(session, payload, "http://localhost:3030/stop_cast")) {
            switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error sending stop to mcs.\n");
        }
        tech_pvt->responseHandler(session, EVENT_CAST_STOP, tech_pvt->seq, address, payload);
        if(switch_hashtable_count(tech_pvt->client_address_hash) == 0){
            status = audio_cast_session_cleanup(session, bugname, 0);
        }
    }
    return status;
}

static switch_status_t start_capture(switch_core_session_t *session, 
        switch_media_bug_flag_t flags, 
        int sampling,
        char* bugname,
        char* payload)
{
    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug;
    switch_status_t status;
    switch_codec_t* read_codec;
    const char* address = cJSON_GetObjectCstr(cJSON_Parse(payload), "address");

    void *pUserData = NULL;
    int channels = (flags & SMBF_STEREO) ? 2 : 1;
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_INFO, 
    "mod_audio_stream (%s): streaming %d sampling.\n", 
    bugname, sampling);

    if ((bug = (switch_media_bug_t*)switch_channel_get_private(channel, bugname))) {
        private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
        int is_addreess_exists = switch_core_hash_find(tech_pvt->client_address_hash, address) != NULL;
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "calling audio_cast_call_mcs.\n");

        if (!is_addreess_exists 
        && SWITCH_STATUS_SUCCESS == switch_core_hash_insert(tech_pvt->client_address_hash, address, address) 
        && SWITCH_STATUS_FALSE == audio_cast_call_mcs(session, payload, "http://localhost:3030/start_cast")) {
            switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error sending start to mcs.\n");
            return SWITCH_STATUS_FALSE;
        }
        tech_pvt->responseHandler(session, EVENT_CAST_START, tech_pvt->seq, address, payload);
        return SWITCH_STATUS_SUCCESS;
    }

    read_codec = switch_core_session_get_read_codec(session);

    if (switch_channel_pre_answer(channel) != SWITCH_STATUS_SUCCESS) {
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "mod_audio_cast: channel must have reached pre-answer status before calling start!\n");
        return SWITCH_STATUS_FALSE;
    }

    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "calling audio_cast_session_init.\n");
    if (SWITCH_STATUS_FALSE == audio_cast_session_init(session, responseHandler, read_codec->implementation->actual_samples_per_second, 
        sampling, channels, bugname, &pUserData)) {
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error initializing mod_audio_cast session.\n");
        return SWITCH_STATUS_FALSE;
    }

    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "calling audio_cast_call_mcs.\n");
    if (SWITCH_STATUS_FALSE == audio_cast_call_mcs(session, payload, "http://localhost:3030/start_cast")) {
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error sending start to mcs.\n");
        return SWITCH_STATUS_FALSE;
    }
    
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "adding bug %s.\n", bugname);
    if ((status = switch_core_media_bug_add(session, bugname, NULL, capture_callback, pUserData, 0, flags, &bug)) != SWITCH_STATUS_SUCCESS) {
        return status;
    }
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "setting bug private data %s.\n", bugname);
    switch_channel_set_private(channel, bugname, bug);
    {
        private_t* tech_pvt = (private_t*) switch_core_media_bug_get_user_data(bug);
        switch_core_hash_insert(tech_pvt->client_address_hash, address, address);
        tech_pvt->responseHandler(session, EVENT_CAST_START, tech_pvt->seq, address, payload);
    }
    switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_DEBUG, "exiting start_capture.\n");
    return SWITCH_STATUS_SUCCESS;
}


static switch_status_t do_send(switch_core_session_t *session, char* bugname, char* payload)
{
    switch_status_t status = SWITCH_STATUS_SUCCESS;

    switch_channel_t *channel = switch_core_session_get_channel(session);
    switch_media_bug_t *bug = (switch_media_bug_t*) switch_channel_get_private(channel, bugname);
    if (!bug) {
      switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "audio_cast_session_send failed because no bug\n");
      return SWITCH_STATUS_FALSE;
    }

    if (SWITCH_STATUS_FALSE == audio_cast_call_mcs(session, payload, "http://localhost:3030/dispatch_event")) {
        switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "Error Sending event to mcs.\n");
        return SWITCH_STATUS_FALSE;
    }

    return status;
}

#define CAST_API_SYNTAX "<uuid> [start | stop | pause | resume | mask | unmask | send]"
SWITCH_STANDARD_API(cast_function)
{
    char *mycmd = NULL, *argv[3] = { 0 };
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
                status = do_stop(lsession, bugname, argv[2] );
              }
            else if (!strcasecmp(argv[1], "pause")) {
                status = do_pauseresume(lsession, bugname, 1);
            }
            else if (!strcasecmp(argv[1], "resume")) {
                status = do_pauseresume(lsession, bugname, 0);
            }
            else if (!strcasecmp(argv[1], "mask")) {
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
                status = start_capture(lsession, flags, sampling, bugname, argv[2]);
            }
            else if (!strcasecmp(argv[1], "send")) {
                status = do_send(lsession, bugname, argv[2]);
            }
            else {
                switch_log_printf(SWITCH_CHANNEL_SESSION_LOG(session), SWITCH_LOG_ERROR, "unsupported mod_audio_cast cmd: %s %s\n", argv[1], argv[2]);
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

	/* create/register custom event message types */
	if (switch_event_reserve_subclass(EVENT_CAST_START) != SWITCH_STATUS_SUCCESS ||
    switch_event_reserve_subclass(EVENT_CAST_STOP) != SWITCH_STATUS_SUCCESS ||
    switch_event_reserve_subclass(EVENT_CAST_PAUSE) != SWITCH_STATUS_SUCCESS ||
    switch_event_reserve_subclass(EVENT_CAST_RESUME) != SWITCH_STATUS_SUCCESS ||
    switch_event_reserve_subclass(EVENT_CAST_MASK) != SWITCH_STATUS_SUCCESS ||
    switch_event_reserve_subclass(EVENT_CAST_UNMASK) != SWITCH_STATUS_SUCCESS ||
    switch_event_reserve_subclass(EVENT_CAST_CLOSE) != SWITCH_STATUS_SUCCESS) {

		switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_ERROR, "Couldn't register an event subclass for mod_audio_cast API.\n");
		return SWITCH_STATUS_TERM;
	}

    SWITCH_ADD_API(api_interface, "uuid_audio_cast", "audio_cast API", cast_function, CAST_API_SYNTAX);
    switch_console_set_complete("add uuid_audio_cast start");
    switch_console_set_complete("add uuid_audio_cast stop");
    switch_console_set_complete("add uuid_audio_cast pause");
    switch_console_set_complete("add uuid_audio_cast resume");
    switch_console_set_complete("add uuid_audio_cast mask");
    switch_console_set_complete("add uuid_audio_cast unmask");
    switch_console_set_complete("add uuid_audio_cast send");
    
    audio_cast_init();

    switch_log_printf(SWITCH_CHANNEL_LOG, SWITCH_LOG_NOTICE, "mod_audio_cast API successfully loaded\n");

    return SWITCH_STATUS_SUCCESS;
}
/*
  Called when the system shuts down
  Macro expands to: switch_status_t mod_audio_cast_shutdown() */
SWITCH_MODULE_SHUTDOWN_FUNCTION(mod_audio_cast_shutdown)
{
    audio_cast_cleanup();

    switch_event_free_subclass(EVENT_CAST_START);
	switch_event_free_subclass(EVENT_CAST_STOP);
	switch_event_free_subclass(EVENT_CAST_PAUSE);
	switch_event_free_subclass(EVENT_CAST_RESUME);
	switch_event_free_subclass(EVENT_CAST_MASK);
	switch_event_free_subclass(EVENT_CAST_UNMASK);
	switch_event_free_subclass(EVENT_CAST_CLOSE);

    return SWITCH_STATUS_SUCCESS;
}
