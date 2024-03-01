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
- `payload` - mcs payload to start a stream {"uuid":"<uuid>", "address":"<address>", "mode":"combined/split-mulaw", "metadata":"<metadata json>"}

##### Freeswitch event generated
**Name**: mod_audio_cast::start
**Body**: JSON string
```
{
  "Client-Address": "address passed on payload",
  "duration_ms": "0",
  "Payload": "<Complete payload passed on command>"
}
```

```
uuid_audio_cast <uuid> stop {payload}
```
Detaches media bug and stop streaming.
- `uuid` - unique identifier of Freeswitch channel
- `payload` - mcs payload to stop a stream to given address on payload {"uuid":"<uuid>", "address":"<address>", "metadata":"<metadata json>"}
- if it is last client, it stop stream to mcs.


##### Freeswitch event generated
**Name**: mod_audio_cast::stop
**Body**: JSON string
```
{
  "Client-Address": "address passed on payload",
  "duration_ms": "<total duration in Millisecond since recording start to stop>",
  "Payload": "<Complete payload passed on command>"
}
```

```
uuid_audio_cast <uuid> pause
```
pause streaming.
##### Freeswitch event generated
**Name**: mod_audio_cast::pause
**Body**: JSON string
```
{
  "Client-Address": "address passed on payload",
  "duration_ms": "<total duration in Millisecond since recording start to pause>"
}
```


```
uuid_audio_cast <uuid> resume
```
resume streaming.
##### Freeswitch event generated
**Name**: mod_audio_cast::resume
**Body**: JSON string
```
{
  "Client-Address": "address passed on payload",
  "duration_ms": "<total duration in Millisecond since recording start to pause>"
}
```

```
uuid_audio_cast <uuid> mask
```
mask with silent packets.
##### Freeswitch event generated
**Name**: mod_audio_cast::mask
**Body**: JSON string
```
{
  "Client-Address": "address passed on payload",
  "duration_ms": "<total duration in Millisecond since recording start to mask>"
}
```

```
uuid_audio_cast <uuid> unmask
```
unmaks streaming.

##### Freeswitch event generated
**Name**: mod_audio_cast::mask
**Body**: JSON string
```
{
  "Client-Address": "address passed on payload",
  "duration_ms": "<total duration in Millisecond since recording start to unmask>"
}
```

```
uuid_audio_cast <uuid> send {event_payload}
```
send event to grpc client 

- `event_payload` - event playload need to send to grpc client. {"uuid":"<uuid>", "event_data":"<event data json>"}
##### Freeswitch event generated
**Name**: mod_audio_cast::mask
**Body**: JSON string
```
{
  "duration_ms": "<total duration in Millisecond since recording start to event sent>",
  "Payload": "<Complete event_payload passed on command>"
}
```

##### Freeswitch event generated
**Name**: mod_audio_cast::close
**Description**: This will get sent either hangup or all subscriber asked to stop.
**Body**: JSON string
```
{
  "duration_ms": "<total duration in Millisecond since recording start to stopped streaming>",
}
```