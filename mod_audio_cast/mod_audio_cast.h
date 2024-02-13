#ifndef __MOD_CAST_H__
#define __MOD_CAST_H__

#include <switch.h>
#include <speex/speex_resampler.h>

#define MAX_BUG_LEN (64)
#define MAX_SESSION_ID (256)
#define MAX_URL_LEN (64)


struct private_data {
	switch_mutex_t *mutex;
	char sessionId[MAX_SESSION_ID];
  char bugname[MAX_BUG_LEN+1];
  char mcsurl[MAX_URL_LEN+1];
  SpeexResamplerState *resampler;
  void * disp;
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
