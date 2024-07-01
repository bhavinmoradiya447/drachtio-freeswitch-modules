# Multi Cast Streamer(MCS)

* [Introduction](#introduction)
* [Architecture](#architecture)
* [Protobuf](#protobuf)
* [GRPC Payload Contract](#grpc-payload-contract)

## Introduction

Multi Cast Streamer(MCS) is built using Rust language and running inside Freeswitch VM. 

Responsibilities of MCS include:

- Get Stereo audio Stream over Domain Socket from FS and Stream it to multiple subscribers over GRPC.
  - If Subscriber need Mono with split channel, it split Stereo to two mono channel and send to  subscribers over GRPC.
- Talk to FS over ESL socket to send event Start/Stop/Failed or any event comming from subscriber.

## Architecture

![image](../docs/src/images/MCS-architecture.png)

## Protobuf 

```
syntax = "proto3";

package mcs;

service MediaCastService {
    rpc Dialog (stream DialogRequestPayload) returns (stream DialogResponsePayload);
}

enum DialogRequestPayloadType { // Request payload type. 
    AUDIO_COMBINED = 0; // This means Payload contains Stereo Audio under field audio and audio_left,audio_right will be empty
    AUDIO_SPLIT = 1; //This means Payload contains Mono Audio under audio_left,audio_right and audio field will be empty
    AUDIO_START = 2; //This indicate start of Stream and event_data will contains intial MetaData. this will not have any audio bytes 
    AUDIO_STOP = 3;  //This indicate that stop requested for subscriber before hangup. hence mcs stopping stream to that subscriber. event_data will contains final MetaData sent with stop request. this will not have any audio bytes 
    AUDIO_END = 4;  //This indicate that end of stream due to hangup or all subscriber has request stop before hangup. this will not have any audio bytes 
    EVENT_DATA = 5; // this indicate that media server has request to send event data to subscriber. event_data contains events and this will not have any audio bytes 
}

message  DialogRequestPayload {
    string uuid = 1; // call leg id
    DialogRequestPayloadType payload_type = 2; // type of payload
    uint64 timestamp = 3; // time stamp when audio was captured
    bytes audio = 4; // audio bytes, in combined stereo format
    bytes audio_left = 5; // audio bytes, left channel
    bytes audio_right = 6; // audio bytes, right channel
    string event_data = 7; // event metadata
}

enum DialogResponsePayloadType {
    EVENT = 0; // DialogResponse has event data
    AUDIO_CHUNK = 1; // DialogResponse has audio chunk
    END_OF_AUDIO = 2; // all audio chunks sent
    DIALOG_END = 3; // end of dialog, event_data may contain message with cause
    RESPONSE_END = 4; // end of response stream
    DIALOG_START = 5; // start of dialog, data may contain message, server should send this to ack.
}

message DialogResponsePayload {
    DialogResponsePayloadType payload_type = 1;
    string data = 2; // multiple purposes, like error message, event data, etc
    bytes audio = 3; // audio chunk
}
```

## GRPC Payload Contract


* MCS will send Payload with type AUDIO_START and initial Metadata as first packet to Subscriber. 
   * Subscriber must send response with payload type either of below three.
        * DIALOG_END : If subscriber find any issue and want to close streaming. they can send cause on data field.
        * DIALOG_START: Subcriber has accept this stream and want to keep Response stream open for sending event/audio back to MCS, so that MCS keep monitoring response stream for future action.
        * RESPONSE_END: Subcriber has accept this stream and dont want to send event/audio back to MCS. so that MCS will not monitor  response stream for future action.
* MCS will send Payload with type AUDIO_COMBINED for Stereo stream or AUDIO_SPLIT and stream left and right channel on field audio_left and audio_right. 
* MCS will send payload with type EVENT_DATA and data willl be there on event_data field (this will not contains any audio bytes) when any event need to be send to subscriber on fly. 
* MCS will send Payload with type AUDIO_STOP and final Metadata when stop requested. 
* MCS will send Payload with type AUDIO_END if call hangup or all subscriber requested for close. 


* Subscriber can send below Response payload if it ack AUDIO_START request with DIALOG_START 
  * Response Payload type as EVENT when Subscriber want to send some events (in data field) back to MCS/MediaServer. 
  *  Response Payload type as AUDIO_CHUNK when Subscriber want to send Aduio file to MCS/Media Server. 
     * Payload type must be AUDIO_CHUNK and audio field should contains audio bytes and data field with below json payload. 
     * Once Done sending Audio byte, Subscriber must send Payload with type END_OF_AUDIO and data  with below json payload and should not contains aduio bytes(if you send as part of END_OF_AUDIO, it will get ignore).
     ```
     {
      "file_name": "<Name of file without extenstion>"
      "action" : "<Action for above file that get send to ras as part of event>"
      "meta_data" : "<meta_data for above file/action as json string>"
     }
     ```
     
Note: Subscriber is GRPC Server to which MCS stream dialog. 