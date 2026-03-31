import { writeFileSync } from 'fs';
import { deflateSync } from 'zlib';

function makePNG(w, h) {
  const buf = Buffer.alloc(8 + 25 + 12 + (1 + w * 4) * h + 500 + 12);
  let off = 0;
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(buf, off); off += 8;
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 6; // 6 = RGBA
  off = writeChunk(buf, off, 'IHDR', ihdr);
  const raw = Buffer.alloc((1 + w * 4) * h);
  for (let y = 0; y < h; y++) {
    const rowOff = y * (1 + w * 4);
    raw[rowOff] = 0; // filter byte
    for (let x = 0; x < w; x++) {
      raw[rowOff + 1 + x * 4] = 0x1f;     // R
      raw[rowOff + 2 + x * 4] = 0x6f;     // G
      raw[rowOff + 3 + x * 4] = 0xeb;     // B
      raw[rowOff + 4 + x * 4] = 0xff;     // A
    }
  }
  off = writeChunk(buf, off, 'IDAT', deflateSync(raw));
  off = writeChunk(buf, off, 'IEND', Buffer.alloc(0));
  return buf.subarray(0, off);
}

function writeChunk(buf, off, type, data) {
  buf.writeUInt32BE(data.length, off); off += 4;
  buf.write(type, off, 4, 'ascii'); off += 4;
  data.copy(buf, off); off += data.length;
  const crcBuf = Buffer.alloc(4 + data.length);
  crcBuf.write(type, 0, 4, 'ascii');
  data.copy(crcBuf, 4);
  buf.writeInt32BE(crc32(crcBuf), off); off += 4;
  return off;
}

function crc32(buf) {
  let crc = 0xFFFFFFFF;
  for (let i = 0; i < buf.length; i++) {
    crc ^= buf[i];
    for (let j = 0; j < 8; j++) crc = (crc >>> 1) ^ (crc & 1 ? 0xEDB88320 : 0);
  }
  return (crc ^ 0xFFFFFFFF) | 0;
}

const dir = 'src-tauri/icons';
[['icon.png', 32], ['32x32.png', 32], ['128x128.png', 128], ['128x128@2x.png', 256]].forEach(([name, size]) => {
  writeFileSync(`${dir}/${name}`, makePNG(size, size));
});
console.log('icons created');
