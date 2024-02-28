# mod_audio_cast

A Freeswitch module that attaches a bug to a media server endpoint and streams mulaw 8000 hz audio to mcs.

## API

### Commands
The freeswitch module exposes the following API commands:

```
uuid_audio_cast <uuid> start {payload}
```
Attaches media bug and starts streaming audio stream to mcs.  Audio is streamed in mu-law format with two channels and 8k hz sample rate. 
- `uuid` - unique identifier of Freeswitch channel
- `payload` - mcs payload to start a stream {"uuid":"<uuid>", "address":"<address>", "meta_data":"<metadata json>"}

```
uuid_audio_cast <uuid> stop {payload}
```
Detaches media bug and stop streaming.
- `uuid` - unique identifier of Freeswitch channel
- `payload` - mcs payload to stop a stream to given address on payload {"uuid":"<uuid>", "address":"<address>", "meta_data":"<metadata json>"}
- if it is last client, it stop stream to mcs.

```
uuid_audio_cast <uuid> pause
```
pause streaming.

```
uuid_audio_cast <uuid> resume
```
resume streaming.

```
uuid_audio_cast <uuid> mask
```
mask with silent packets.


```
uuid_audio_cast <uuid> unmask
```
unmaks streaming.

```
uuid_audio_cast <uuid> send {event_payload}
```
send event to grpc client 

- `event_payload` - event playload need to send to grpc client. {"uuid":"<uuid>", "event_data":"<event data json>"}
