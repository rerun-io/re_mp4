import fs from "node:fs";

const __dirname = new URL(".", import.meta.url).pathname;

const a = "sintel_trailer-720p-h264.mp4.track_1.bin";
const b = "sintel_trailer-720p-h264.bin";

const aData = new Uint8Array(fs.readFileSync(__dirname + "/samples/" + a));
const bData = new Uint8Array(fs.readFileSync(__dirname + "/samples/" + b));

if (aData.length !== bData.length) {
  throw new Error("file sizes differ");
}

for (let i = 0; i < aData.length; i++) {
  if (aData[i] !== bData[i]) {
    throw new Error("files differ at byte " + i);
  }
}

console.log("success");

