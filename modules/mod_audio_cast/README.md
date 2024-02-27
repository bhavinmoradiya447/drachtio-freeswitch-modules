# mod_audio_stream

A Freeswitch module that attaches a bug to a media server endpoint and streams L16 audio to NamedPipe.

## API

### Commands
The freeswitch module exposes the following API commands:

```
uuid_audio_fork <uuid> start {payload}
```
Attaches media bug and starts streaming audio stream to NamedPipe.  Audio is streamed in mu-law format with two channels and 8k hz sample rate. 
- `uuid` - unique identifier of Freeswitch channel
- `payload` - mcs payload to start a stream {"uuid":"<uuid>", "address":"<address>", "metadata":"<metadata>"}

```
uuid_audio_fork <uuid> stop {payload}
```
Detaches media bug and stop streaming.
- `uuid` - unique identifier of Freeswitch channel
- `payload` - mcs payload to stop a stream to given address on payload {"uuid":"<uuid>", "address":"<address>", "metadata":"<metadata>"}
- if it is last client, it stop stream to mcs.

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

```
uuid_audio_fork <uuid> send {event_payload}
```
send event to grpc client 

- `event_payload` - event playload need to send to grpc client
