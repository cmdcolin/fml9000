// Symphonia
// Copyright (c) 2019-2022 The Project Symphonia Developers.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::{Error, Result};
use symphonia::core::formats::{Cue, FormatOptions, FormatReader, SeekMode, SeekTo, Track};
use symphonia::core::io::{MediaSource, MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::{ColorMode, MetadataOptions, MetadataRevision, Tag, Value, Visual};
use symphonia::core::probe::{Hint, ProbeResult};
use symphonia::core::units::{Time, TimeBase};

use log::{error, info, warn};

mod output;

pub fn play_track(path_str: String) {
  // Create a hint to help the format registry guess what format reader is appropriate.
  let mut hint = Hint::new();

  // If the path string is '-' then read from standard input.
  let source = if path_str == "-" {
    Box::new(ReadOnlySource::new(std::io::stdin())) as Box<dyn MediaSource>
  } else {
    // Othwerise, get a Path from the path string.
    let path = Path::new(&path_str);

    // Provide the file extension as a hint.
    if let Some(extension) = path.extension() {
      if let Some(extension_str) = extension.to_str() {
        hint.with_extension(extension_str);
      }
    }

    Box::new(File::open(path).unwrap())
  };

  // Create the media source stream using the boxed media source from above.
  let mss = MediaSourceStream::new(source, Default::default());

  // Use the default options for format readers other than for gapless playback.
  let format_opts = FormatOptions {
    enable_gapless: true,
    ..Default::default()
  };

  // Use the default options for metadata readers.
  let metadata_opts: MetadataOptions = Default::default();

  let no_progress = true;

  // Probe the media source stream for metadata and get the format reader.
  match symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts) {
    Ok(mut probed) => {
      let result = if false {
        // Verify-only mode decodes and verifies the audio, but does not play it.
        decode_only(
          probed.format,
          &DecoderOptions {
            verify: true,
            ..Default::default()
          },
        )
      } else if false {
        // Decode-only mode decodes the audio, but does not play or verify it.
        decode_only(
          probed.format,
          &DecoderOptions {
            verify: false,
            ..Default::default()
          },
        )
      } else if false {
        // Probe-only mode only prints information about the format, tracks, metadata, etc.
        print_format(&path_str, &mut probed);
        Ok(())
      } else {
        // Playback mode.
        print_format(&path_str, &mut probed);

        // If present, parse the seek argument.
        let seek_time = Some(0.0);
        let track = Some(0);

        // Set the decoder options.
        let decode_opts = DecoderOptions {
          verify: false,
          ..Default::default()
        };

        // Play it!
        play(probed.format, track, seek_time, &decode_opts, no_progress)
      };

      if let Err(err) = result {
        error!("error: {}", err);
      }
    }
    Err(err) => {
      // The input was not supported by any format reader.
      error!("file not supported. reason? {}", err);
    }
  }
}

fn decode_only(mut reader: Box<dyn FormatReader>, decode_opts: &DecoderOptions) -> Result<()> {
  // Get the default track.
  // TODO: Allow track selection.
  let track = reader.default_track().unwrap();
  let track_id = track.id;

  // Create a decoder for the track.
  let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, decode_opts)?;

  // Decode all packets, ignoring all decode errors.
  let result = loop {
    let packet = match reader.next_packet() {
      Ok(packet) => packet,
      Err(err) => break Err(err),
    };

    // If the packet does not belong to the selected track, skip over it.
    if packet.track_id() != track_id {
      continue;
    }

    // Decode the packet into audio samples.
    match decoder.decode(&packet) {
      Ok(_decoded) => continue,
      Err(Error::DecodeError(err)) => warn!("decode error: {}", err),
      Err(err) => break Err(err),
    }
  };

  // Regardless of result, finalize the decoder to get the verification result.
  let finalize_result = decoder.finalize();

  if let Some(verify_ok) = finalize_result.verify_ok {
    if verify_ok {
      info!("verification passed");
    } else {
      info!("verification failed");
    }
  }

  result
}

#[derive(Copy, Clone)]
struct PlayTrackOptions {
  track_id: u32,
  seek_ts: u64,
}

fn play(
  mut reader: Box<dyn FormatReader>,
  track_num: Option<usize>,
  seek_time: Option<f64>,
  decode_opts: &DecoderOptions,
  no_progress: bool,
) -> Result<()> {
  // If the user provided a track number, select that track if it exists, otherwise, select the
  // first track with a known codec.
  let track = track_num
    .and_then(|t| reader.tracks().get(t))
    .or_else(|| first_supported_track(reader.tracks()));

  let mut track_id = match track {
    Some(track) => track.id,
    _ => return Ok(()),
  };

  // If there is a seek time, seek the reader to the time specified and get the timestamp of the
  // seeked position. All packets with a timestamp < the seeked position will not be played.
  //
  // Note: This is a half-baked approach to seeking! After seeking the reader, packets should be
  // decoded and *samples* discarded up-to the exact *sample* indicated by required_ts. The
  // current approach will discard excess samples if seeking to a sample within a packet.
  let seek_ts = if let Some(time) = seek_time {
    let seek_to = SeekTo::Time {
      time: Time::from(time),
      track_id: Some(track_id),
    };

    // Attempt the seek. If the seek fails, ignore the error and return a seek timestamp of 0 so
    // that no samples are trimmed.
    match reader.seek(SeekMode::Accurate, seek_to) {
      Ok(seeked_to) => seeked_to.required_ts,
      Err(Error::ResetRequired) => {
        print_tracks(reader.tracks());
        track_id = first_supported_track(reader.tracks()).unwrap().id;
        0
      }
      Err(err) => {
        // Don't give-up on a seek error.
        warn!("seek error: {}", err);
        0
      }
    }
  } else {
    // If not seeking, the seek timestamp is 0.
    0
  };

  // The audio output device.
  let mut audio_output = None;

  let mut track_info = PlayTrackOptions { track_id, seek_ts };

  let result = loop {
    match play_audio(
      &mut reader,
      &mut audio_output,
      track_info,
      decode_opts,
      no_progress,
    ) {
      Err(Error::ResetRequired) => {
        // The demuxer indicated that a reset is required. This is sometimes seen with
        // streaming OGG (e.g., Icecast) wherein the entire contents of the container change
        // (new tracks, codecs, metadata, etc.). Therefore, we must select a new track and
        // recreate the decoder.
        print_tracks(reader.tracks());

        // Select the first supported track since the user's selected track number might no
        // longer be valid or make sense.
        let track_id = first_supported_track(reader.tracks()).unwrap().id;
        track_info = PlayTrackOptions {
          track_id,
          seek_ts: 0,
        };
      }
      res => break res,
    }
  };

  // Flush the audio output to finish playing back any leftover samples.
  if let Some(audio_output) = audio_output.as_mut() {
    audio_output.flush()
  }

  result
}

fn play_audio(
  reader: &mut Box<dyn FormatReader>,
  audio_output: &mut Option<Box<dyn output::AudioOutput>>,
  play_opts: PlayTrackOptions,
  decode_opts: &DecoderOptions,
  no_progress: bool,
) -> Result<()> {
  // Get the selected track using the track ID.
  let track = match reader
    .tracks()
    .iter()
    .find(|track| track.id == play_opts.track_id)
  {
    Some(track) => track,
    _ => return Ok(()),
  };

  // Create a decoder for the track.
  let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, decode_opts)?;

  // Get the selected track's timebase and duration.
  let tb = track.codec_params.time_base;
  let dur = track
    .codec_params
    .n_frames
    .map(|frames| track.codec_params.start_ts + frames);

  // Decode and play the packets belonging to the selected track.
  let result = loop {
    // Get the next packet from the format reader.
    let packet = match reader.next_packet() {
      Ok(packet) => packet,
      Err(err) => break Err(err),
    };

    // If the packet does not belong to the selected track, skip it.
    if packet.track_id() != play_opts.track_id {
      continue;
    }

    //Print out new metadata.
    while !reader.metadata().is_latest() {
      reader.metadata().pop();

      if let Some(rev) = reader.metadata().current() {
        print_update(rev);
      }
    }

    // Decode the packet into audio samples.
    match decoder.decode(&packet) {
      Ok(decoded) => {
        // If the audio output is not open, try to open it.
        if audio_output.is_none() {
          // Get the audio buffer specification. This is a description of the decoded
          // audio buffer's sample format and sample rate.
          let spec = *decoded.spec();

          // Get the capacity of the decoded buffer. Note that this is capacity, not
          // length! The capacity of the decoded buffer is constant for the life of the
          // decoder, but the length is not.
          let duration = decoded.capacity() as u64;

          // Try to open the audio output.
          audio_output.replace(output::try_open(spec, duration).unwrap());
        } else {
          // TODO: Check the audio spec. and duration hasn't changed.
        }

        // Write the decoded audio samples to the audio output if the presentation timestamp
        // for the packet is >= the seeked position (0 if not seeking).
        if packet.ts() >= play_opts.seek_ts {
          if !no_progress {
            print_progress(packet.ts(), dur, tb);
          }

          if let Some(audio_output) = audio_output {
            audio_output.write(decoded).unwrap()
          }
        }
      }
      Err(Error::DecodeError(err)) => {
        // Decode errors are not fatal. Print the error message and try to decode the next
        // packet as usual.
        warn!("decode error: {}", err);
      }
      Err(err) => break Err(err),
    }
  };

  // Regardless of result, finalize the decoder to get the verification result.
  let finalize_result = decoder.finalize();

  if let Some(verify_ok) = finalize_result.verify_ok {
    if verify_ok {
      info!("verification passed");
    } else {
      info!("verification failed");
    }
  }

  result
}

fn first_supported_track(tracks: &[Track]) -> Option<&Track> {
  tracks
    .iter()
    .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
}

fn print_format(path: &str, probed: &mut ProbeResult) {
  println!("+ {}", path);
  print_tracks(probed.format.tracks());

  // Prefer metadata that's provided in the container format, over other tags found during the
  // probe operation.
  if let Some(metadata_rev) = probed.format.metadata().current() {
    print_tags(metadata_rev.tags());
    print_visuals(metadata_rev.visuals());

    // Warn that certain tags are preferred.
    if probed.metadata.get().as_ref().is_some() {
      info!("tags that are part of the container format are preferentially printed.");
      info!("not printing additional tags that were found while probing.");
    }
  } else if let Some(metadata_rev) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
    print_tags(metadata_rev.tags());
    print_visuals(metadata_rev.visuals());
  }

  print_cues(probed.format.cues());
  println!(":");
  println!();
}

fn print_update(rev: &MetadataRevision) {
  print_tags(rev.tags());
  print_visuals(rev.visuals());
  println!(":");
  println!();
}

fn print_tracks(tracks: &[Track]) {
  if !tracks.is_empty() {
    println!("|");
    println!("| // Tracks //");

    for (idx, track) in tracks.iter().enumerate() {
      let params = &track.codec_params;

      print!("|     [{:0>2}] Codec:           ", idx + 1);

      if let Some(codec) = symphonia::default::get_codecs().get_codec(params.codec) {
        println!("{} ({})", codec.long_name, codec.short_name);
      } else {
        println!("Unknown (#{})", params.codec);
      }

      if let Some(sample_rate) = params.sample_rate {
        println!("|          Sample Rate:     {}", sample_rate);
      }
      if params.start_ts > 0 {
        if let Some(tb) = params.time_base {
          println!(
            "|          Start Time:      {} ({})",
            fmt_time(params.start_ts, tb),
            params.start_ts
          );
        } else {
          println!("|          Start Time:      {}", params.start_ts);
        }
      }
      if let Some(n_frames) = params.n_frames {
        if let Some(tb) = params.time_base {
          println!(
            "|          Duration:        {} ({})",
            fmt_time(n_frames, tb),
            n_frames
          );
        } else {
          println!("|          Frames:          {}", n_frames);
        }
      }
      if let Some(tb) = params.time_base {
        println!("|          Time Base:       {}", tb);
      }
      if let Some(padding) = params.delay {
        println!("|          Encoder Delay:   {}", padding);
      }
      if let Some(padding) = params.padding {
        println!("|          Encoder Padding: {}", padding);
      }
      if let Some(sample_format) = params.sample_format {
        println!("|          Sample Format:   {:?}", sample_format);
      }
      if let Some(bits_per_sample) = params.bits_per_sample {
        println!("|          Bits per Sample: {}", bits_per_sample);
      }
      if let Some(channels) = params.channels {
        println!("|          Channel(s):      {}", channels.count());
        println!("|          Channel Map:     {}", channels);
      }
      if let Some(channel_layout) = params.channel_layout {
        println!("|          Channel Layout:  {:?}", channel_layout);
      }
      if let Some(language) = &track.language {
        println!("|          Language:        {}", language);
      }
    }
  }
}

fn print_cues(cues: &[Cue]) {
  if !cues.is_empty() {
    println!("|");
    println!("| // Cues //");

    for (idx, cue) in cues.iter().enumerate() {
      println!("|     [{:0>2}] Track:      {}", idx + 1, cue.index);
      println!("|          Timestamp:  {}", cue.start_ts);

      // Print tags associated with the Cue.
      if !cue.tags.is_empty() {
        println!("|          Tags:");

        for (tidx, tag) in cue.tags.iter().enumerate() {
          if let Some(std_key) = tag.std_key {
            println!(
              "{}",
              print_tag_item(tidx + 1, &format!("{:?}", std_key), &tag.value, 21)
            );
          } else {
            println!("{}", print_tag_item(tidx + 1, &tag.key, &tag.value, 21));
          }
        }
      }

      // Print any sub-cues.
      if !cue.points.is_empty() {
        println!("|          Sub-Cues:");

        for (ptidx, pt) in cue.points.iter().enumerate() {
          println!(
            "|                      [{:0>2}] Offset:    {:?}",
            ptidx + 1,
            pt.start_offset_ts
          );

          // Start the number of sub-cue tags, but don't print them.
          if !pt.tags.is_empty() {
            println!(
              "|                           Sub-Tags:  {} (not listed)",
              pt.tags.len()
            );
          }
        }
      }
    }
  }
}

fn print_tags(tags: &[Tag]) {
  if !tags.is_empty() {
    println!("|");
    println!("| // Tags //");

    let mut idx = 1;

    // Print tags with a standard tag key first, these are the most common tags.
    for tag in tags.iter().filter(|tag| tag.is_known()) {
      if let Some(std_key) = tag.std_key {
        println!(
          "{}",
          print_tag_item(idx, &format!("{:?}", std_key), &tag.value, 4)
        );
      }
      idx += 1;
    }

    // Print the remaining tags with keys truncated to 26 characters.
    for tag in tags.iter().filter(|tag| !tag.is_known()) {
      println!("{}", print_tag_item(idx, &tag.key, &tag.value, 4));
      idx += 1;
    }
  }
}

fn print_visuals(visuals: &[Visual]) {
  if !visuals.is_empty() {
    println!("|");
    println!("| // Visuals //");

    for (idx, visual) in visuals.iter().enumerate() {
      if let Some(usage) = visual.usage {
        println!("|     [{:0>2}] Usage:      {:?}", idx + 1, usage);
        println!("|          Media Type: {}", visual.media_type);
      } else {
        println!("|     [{:0>2}] Media Type: {}", idx + 1, visual.media_type);
      }
      if let Some(dimensions) = visual.dimensions {
        println!(
          "|          Dimensions: {} px x {} px",
          dimensions.width, dimensions.height
        );
      }
      if let Some(bpp) = visual.bits_per_pixel {
        println!("|          Bits/Pixel: {}", bpp);
      }
      if let Some(ColorMode::Indexed(colors)) = visual.color_mode {
        println!("|          Palette:    {} colors", colors);
      }
      println!("|          Size:       {} bytes", visual.data.len());

      // Print out tags similar to how regular tags are printed.
      if !visual.tags.is_empty() {
        println!("|          Tags:");
      }

      for (tidx, tag) in visual.tags.iter().enumerate() {
        if let Some(std_key) = tag.std_key {
          println!(
            "{}",
            print_tag_item(tidx + 1, &format!("{:?}", std_key), &tag.value, 21)
          );
        } else {
          println!("{}", print_tag_item(tidx + 1, &tag.key, &tag.value, 21));
        }
      }
    }
  }
}

fn print_tag_item(idx: usize, key: &str, value: &Value, indent: usize) -> String {
  let key_str = match key.len() {
    0..=28 => format!("| {:w$}[{:0>2}] {:<28} : ", "", idx, key, w = indent),
    _ => format!(
      "| {:w$}[{:0>2}] {:.<28} : ",
      "",
      idx,
      key.split_at(26).0,
      w = indent
    ),
  };

  let line_prefix = format!("\n| {:w$} : ", "", w = indent + 4 + 28 + 1);
  let line_wrap_prefix = format!("\n| {:w$}   ", "", w = indent + 4 + 28 + 1);

  let mut out = String::new();

  out.push_str(&key_str);

  for (wrapped, line) in value.to_string().lines().enumerate() {
    if wrapped > 0 {
      out.push_str(&line_prefix);
    }

    let mut chars = line.chars();
    let split = (0..)
      .map(|_| chars.by_ref().take(72).collect::<String>())
      .take_while(|s| !s.is_empty())
      .collect::<Vec<_>>();

    out.push_str(&split.join(&line_wrap_prefix));
  }

  out
}

fn fmt_time(ts: u64, tb: TimeBase) -> String {
  let time = tb.calc_time(ts);

  let hours = time.seconds / (60 * 60);
  let mins = (time.seconds % (60 * 60)) / 60;
  let secs = f64::from((time.seconds % 60) as u32) + time.frac;

  format!("{}:{:0>2}:{:0>6.3}", hours, mins, secs)
}

fn print_progress(ts: u64, dur: Option<u64>, tb: Option<TimeBase>) {}
