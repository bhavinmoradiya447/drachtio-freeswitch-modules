#ifndef __MOD_FORK_H__
#define __MOD_FORK_H__

#include <switch.h>
#include <speex/speex_resampler.h>
#include "dispatcher.h"

#define MY_BUG_NAME "audio_fork"
#define MAX_BUG_LEN (64)
#define MAX_SESSION_ID (256)

typedef void (*responseHandler_t)(switch_core_session_t* session, const char* eventName, char* json);

struct private_data {
	switch_mutex_t *mutex;
	char sessionId[MAX_SESSION_ID];
  char bugname[MAX_BUG_LEN+1];
  SpeexResamplerState *resampler;
  dispatcher * disp;
  int sampling;
  int  channels;
  int seq;
  unsigned int id;
  int audio_masked:1;
  int audio_paused:1;
  int graceful_shutdown:1;
};

typedef struct private_data private_t;

#endif
