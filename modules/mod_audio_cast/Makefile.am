include $(top_srcdir)/build/modmake.rulesam
MODNAME=mod_audio_cast

CC = g++
mod_LTLIBRARIES = mod_audio_cast.la
mod_audio_cast_la_SOURCES  = mod_audio_cast.c audio_cast_helper.cpp dispatcher.cpp
mod_audio_cast_la_CFLAGS   = $(AM_CFLAGS)
mod_audio_stream_la_CXXFLAGS = $(AM_CXXFLAGS) -std=c++11

mod_audio_cast_la_LIBADD   = $(switch_builddir)/libfreeswitch.la
mod_audio_cast_la_LDFLAGS  = -avoid-version -module -no-undefined -shared
