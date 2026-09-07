// A small, locally generated music loop gives first-time visitors an audio source
// without downloading a media asset or sending their own files to a server.
export function createDemoAudioFile(): File {
  const rate = 24_000, seconds = 8, frames = rate * seconds;
  const data = new ArrayBuffer(44 + frames * 2);
  const view = new DataView(data);
  const text = (offset: number, value: string) => {
    for (let i = 0; i < value.length; i++) view.setUint8(offset + i, value.charCodeAt(i));
  };
  text(0, 'RIFF'); view.setUint32(4, data.byteLength - 8, true); text(8, 'WAVEfmt ');
  view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true);
  view.setUint32(24, rate, true); view.setUint32(28, rate * 2, true);
  view.setUint16(32, 2, true); view.setUint16(34, 16, true);
  text(36, 'data'); view.setUint32(40, frames * 2, true);
  const notes = [261.63, 329.63, 392, 493.88, 440, 392, 329.63, 293.66];
  for (let i = 0; i < frames; i++) {
    const time = i / rate, local = time % .25;
    const frequency = notes[Math.floor(time / .25) % notes.length]!;
    const envelope = Math.min(1, local / .008) * Math.exp(-local * 13);
    const tone = Math.sin(2 * Math.PI * frequency * time) + .22 * Math.sin(4 * Math.PI * frequency * time);
    const bass = Math.sin(2 * Math.PI * 65.4075 * time) * Math.exp(-(time % .5) * 8);
    const fade = Math.min(1, time * 20, (seconds - time) * 20);
    view.setInt16(44 + i * 2, Math.round((tone * envelope * .24 + bass * .14) * fade * 32767), true);
  }
  return new File([data], 'WVST — Copper loop.wav', { type: 'audio/wav' });
}

export async function readAudioPeaks(file: File): Promise<number[]> {
  // Large files still play through the media element without decoding a second copy.
  if (file.size > 50 * 1024 * 1024) return [];
  const context = new OfflineAudioContext(1, 1, 24_000);
  const audio = await context.decodeAudioData(await file.arrayBuffer());
  const channel = audio.getChannelData(0);
  const count = 96, width = Math.max(1, Math.floor(channel.length / count));
  return Array.from({ length: count }, (_, index) => {
    let peak = 0;
    for (let i = index * width; i < Math.min((index + 1) * width, channel.length); i++) {
      peak = Math.max(peak, Math.abs(channel[i]!));
    }
    return peak;
  });
}
