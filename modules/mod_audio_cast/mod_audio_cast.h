#ifndef __MOD_CAST_H__
#define __MOD_CAST_H__

#include <switch.h>
#include <switch_resample.h>
#include "private/switch_hashtable_private.h"
#include <switch_json.h>
#define MAX_BUG_LEN (64)
#define MAX_SESSION_ID (256)

#define EVENT_CAST_START     "mod_audio_cast::start"
#define EVENT_CAST_STOP      "mod_audio_cast::stop"
#define EVENT_CAST_PAUSE     "mod_audio_cast::pause"
#define EVENT_CAST_RESUME    "mod_audio_cast::resume"
#define EVENT_CAST_MASK      "mod_audio_cast::mask"
#define EVENT_CAST_UNMASK    "mod_audio_cast::unmask"
#define EVENT_CAST_CLOSE     "mod_audio_cast::close"

typedef void (*responseHandler_t)(switch_core_session_t* session, const char* eventName, int last_seq, const char* address, char* json);

struct private_data {
  switch_mutex_t *mutex;
  char sessionId[MAX_SESSION_ID];
  char bugname[MAX_BUG_LEN+1];
  switch_audio_resampler_t *read_resampler;
  responseHandler_t responseHandler;
  void * disp;
  int sampling;
  int  channels;
  int seq;
  switch_hash_t *client_address_hash;
  unsigned int id;
  int audio_masked:1;
  int audio_paused:1;
  int graceful_shutdown:1;
};

typedef struct private_data private_t;

#endif
