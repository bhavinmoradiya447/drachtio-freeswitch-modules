# mod_audio_stream

A Freeswitch module that attaches a bug to a media server endpoint and streams L16 audio to NamedPipe.

## API

### Commands
The freeswitch module exposes the following API commands:

```
uuid_audio_fork <uuid> start grpc-end-point
```
Attaches media bug and starts streaming audio stream to NamedPipe.  Audio is streamed in linear 16 format (16-bit PCM encoding) with two channels and 8k hz sample rate. 
- `uuid` - unique identifier of Freeswitch channel

```
uuid_audio_fork <uuid> stop
```
Detaches media bug and stop streaming.

```
uuid_audio_fork <uuid> pause
```
pause streaming.


```
uuid_audio_fork <uuid> resume
```
resume streaming.

```
uuid_audio_fork <uuid> mask
```
mask with silent packets.


```
uuid_audio_fork <uuid> unmask
```
unmaks streaming.


