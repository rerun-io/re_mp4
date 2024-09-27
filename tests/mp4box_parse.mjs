// @ts-check

import * as MP4Box from "./mp4box.mjs";
import fs from "node:fs";

/**
 * @typedef {{
 *   type: "key" | "delta",
 *   timestamp: number,
 *   duration: number,
 *   byteOffset: number,
 *   byteLength: number,
 * }} Sample
 *
 * @typedef {{
 *   samples: Sample[],
 *   start: number,
 * }} Segment
 */

/** @param {ArrayBuffer} file */
function unboxVideo(file) {
  const mp4 = MP4Box.createFile();

  let track = {};
  let videoDecoderConfig = /** @type {VideoDecoderConfig} */ (/** @type {unknown} */ (undefined));
  let timescale = 1000;
  let duration = 0;

  mp4.onReady = (info) => {
    track = info.videoTracks[0];

    let description = null;
    const trak = mp4.getTrackById(track.id);
    for (const entry of trak.mdia.minf.stbl.stsd.entries) {
      const box = entry.avcC || entry.hvcC || entry.vpcC || entry.av1C;
      if (box) {
        const stream = new MP4Box.DataStream(undefined, 0, MP4Box.DataStream.BIG_ENDIAN);
        box.write(stream);
        const buffer = /** @type {ArrayBuffer} */ (stream.buffer);
        description = new Uint8Array(buffer, 8); // Remove the box header.
        break;
      }
    }

    if (!description) {
      throw new Error("avcC, hvcC, vpcC, or av1C box not found");
    }

    console.log(track.codec);

    videoDecoderConfig = {
      codec: track.codec.startsWith("vp08") ? "vp8" : track.codec,
      codedHeight: track.video.height,
      codedWidth: track.video.width,
      description,
    };
    timescale = info.timescale;
    duration = info.duration;

    mp4.setExtractionOptions(track.id);
    mp4.start();
  };

  let rawSamples = [];
  mp4.onSamples = (_a, _b, samples) => {
    Array.prototype.push.apply(rawSamples, samples);
  };

  mp4.appendBuffer(Object.assign(file, { fileStart: 0 }));
  mp4.flush();

  /** @type {Segment[]} */
  let segments = [];
  /** @type {Sample[]} */
  let samples = [];
  let data = new ArrayBuffer(mp4.samplesDataSize);
  let view = new Uint8Array(data);
  let byteOffset = 0;
  for (const sample of rawSamples) {
    let byteLength = sample.data.byteLength;
    view.set(sample.data, byteOffset);

    if (sample.is_sync) {
      if (samples.length !== 0) {
        segments.push({
          samples,
          start: samples[0].timestamp,
        });
        samples = [];
      }
    }

    samples.push({
      type: sample.is_sync ? "key" : "delta",
      timestamp: (1e6 * sample.cts) / sample.timescale,
      duration: (1e6 * sample.duration) / sample.timescale,
      byteOffset,
      byteLength,
    });

    byteOffset += byteLength;
  }

  if (samples.length !== 0) {
    segments.push({
      samples,
      start: samples[0].timestamp,
    });
  }

  if (!videoDecoderConfig) {
    // shouldn't happen, the callbacks are synchronous and should
    // be called by the time `flush` is called
    throw new Error("invalid ordering");
  }

  return {
    segments,
    videoDecoderConfig,
    timescale,
    duration,
    data,
  };
}

const file = process.argv[2];
const video = unboxVideo(fs.readFileSync(file).buffer);
console.log(video.videoDecoderConfig);

fs.writeFileSync(file.replace(".mp4", ".bin"), new Uint8Array(video.data));

let timeOffset = -1;
let out = "[";
for (const segment of video.segments) {
  const first_sample = segment.samples[0];
  if (timeOffset < 0) {
    timeOffset = first_sample.timestamp / 1e3;
  }

  out += [
    "\n    Sample {",
    "        is_sync: true,",
    `        size: ${first_sample.byteLength},`,
    `        offset: ${first_sample.byteOffset},`,
    `        timestamp: ${first_sample.timestamp / 1e3 - timeOffset},`,
    `        duration: ${first_sample.duration / 1e3},`,
    "    },",
  ].join("\n");
  for (let i = 1; i < segment.samples.length; i++) {
    const sample = segment.samples[i];
    out += [
      "\n    Sample {",
      "        is_sync: false,",
      `        size: ${sample.byteLength},`,
      `        offset: ${sample.byteOffset},`,
      `        timestamp: ${sample.timestamp / 1e3 - timeOffset},`,
      `        duration: ${sample.duration / 1e3},`,
      "    },",
    ].join("\n");
  }
}
out += "\n]";

fs.writeFileSync(file.replace(".mp4", ".segments"), out);

