#ifndef __CAST_HELPER_H__
#define __CAST_HELPER_H__

#include "mod_audio_cast.h"

int parse_ws_uri(switch_channel_t *channel, const char* szServerUri, char* host, char *path, unsigned int* pPort, int* pSslFlags);

switch_status_t audio_cast_init();
switch_status_t audio_cast_cleanup();
switch_status_t audio_cast_session_init(switch_core_session_t *session, 
    responseHandler_t responseHandler,
    uint32_t samples_per_second, int sampling, int channels, 
    char *bugname, void **ppUserData);
switch_status_t audio_cast_call_mcs(switch_core_session_t *session, char* payload, char* url);
switch_status_t audio_cast_session_cleanup(switch_core_session_t *session, char *bugname, int channelIsClosing);
switch_status_t audio_cast_session_pauseresume(switch_core_session_t *session, char *bugname, int pause);
switch_status_t audio_cast_session_maskunmask(switch_core_session_t *session, char *bugname, int mask);
switch_status_t audio_cast_session_graceful_shutdown(switch_core_session_t *session, char *bugname);
switch_bool_t convert_linear2_g711_pcmu8k(char* frame, uint32_t* framelen);
switch_bool_t audio_cast_frame(switch_core_session_t *session, switch_media_bug_t *bug);
#endif
