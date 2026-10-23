export const referenceImage = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aBmcAAAAASUVORK5CYII=", "base64");

// A self-contained triangle fixture for GLB validation and browser rendering.
export function triangleGlb() {
  const positions = Buffer.alloc(36);
  [-1, 0, 0, 1, 0, 0, 0, 1.5, 0].forEach((n, i) => positions.writeFloatLE(n, i * 4));
  const document = JSON.stringify({ asset: { version: "2.0" }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0 }], meshes: [{ primitives: [{ attributes: { POSITION: 0 } }] }], buffers: [{ byteLength: 36 }], bufferViews: [{ buffer: 0, byteLength: 36 }], accessors: [{ bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [-1,0,0], max: [1,1.5,0] }] });
  const json = Buffer.from(document.padEnd(Math.ceil(document.length / 4) * 4, " "));
  const bytes = Buffer.alloc(12 + 8 + json.length + 8 + positions.length);
  bytes.write("glTF"); bytes.writeUInt32LE(2, 4); bytes.writeUInt32LE(bytes.length, 8);
  bytes.writeUInt32LE(json.length, 12); bytes.writeUInt32LE(0x4e4f534a, 16); json.copy(bytes, 20);
  bytes.writeUInt32LE(positions.length, 20 + json.length); bytes.writeUInt32LE(0x004e4942, 24 + json.length); positions.copy(bytes, 28 + json.length);
  return bytes;
}
