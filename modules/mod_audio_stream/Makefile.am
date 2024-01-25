include $(top_srcdir)/build/modmake.rulesam
MODNAME=mod_audio_stream

mod_LTLIBRARIES = mod_audio_stream.la
mod_audio_stream_la_SOURCES  = mod_audio_stream.c lws_glue.cpp parser.cpp audio_pipe.cpp
mod_audio_stream_la_CFLAGS   = $(AM_CFLAGS)
mod_audio_stream_la_CXXFLAGS = $(AM_CXXFLAGS) -std=c++11 -lzmq

mod_audio_stream_la_LIBADD   = $(switch_builddir)/libfreeswitch.la
mod_audio_stream_la_LDFLAGS  = -avoid-version -module -no-undefined -shared `pkg-config --libs libwebsockets` -lzmq
