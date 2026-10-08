"""Bounded offline PCM mix of original collision layers, without new sounds."""
from array import array
import math
from pathlib import Path
import sys
import wave


def render(sources: list[Path], target: Path) -> dict:
    if not 1 <= len(sources) <= 8:
        raise ValueError('An impact variant must contain 1..8 layers')
    layers = []
    rate = None
    for source in sources:
        with wave.open(str(source), 'rb') as wav:
            if (wav.getnchannels() != 1 or wav.getsampwidth() != 2
                    or wav.getcomptype() != 'NONE' or not 1 <= wav.getframerate() <= 192000
                    or not 1 <= wav.getnframes() <= wav.getframerate() * 10):
                raise ValueError('Impact layers must be bounded mono PCM16')
            if rate is not None and rate != wav.getframerate():
                raise ValueError('Impact layers must share a sample rate')
            rate = wav.getframerate()
            frames = wav.getnframes()
            raw = wav.readframes(frames)
            if len(raw) != frames * 2:
                raise ValueError('Truncated impact layer')
            samples = array('h', raw)
            if sys.byteorder != 'little':
                samples.byteswap()
            layers.append(samples)
    mixed = array('i', [0]) * max(map(len, layers))
    for samples in layers:
        for index, value in enumerate(samples):
            mixed[index] += value
    # Provisional host mix: simultaneous layers, equal-power attenuation, then
    # additional peak attenuation if needed. Never clip or normalize upwards.
    high, low = max(mixed), min(mixed)
    gain = min(1 / math.sqrt(len(layers)),
               32767 / high if high > 0 else 1, -32768 / low if low < 0 else 1)
    pcm = array('h', (round(value * gain) for value in mixed))
    if sys.byteorder != 'little':
        pcm.byteswap()
    target.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(target), 'wb') as wav:
        wav.setparams((1, 2, rate, len(pcm), 'NONE', 'not compressed'))
        wav.writeframes(pcm.tobytes())
    # Measure the loudest 50 ms window, rather than full-clip RMS (which lets
    # long quiet tails disguise an overly loud attack). Attenuate only.
    width = min(len(mixed), max(1, round(rate * 0.05)))
    energy = peak_energy = 0
    for index, value in enumerate(mixed):
        energy += value * value
        if index >= width:
            energy -= mixed[index - width] ** 2
        if index >= width - 1:
            peak_energy = max(peak_energy, energy)
    attack_rms = math.sqrt(peak_energy / width) * gain / 32768
    peak = max(abs(high), abs(low)) * gain / 32768
    trim = min(1., 0.18 / attack_rms if attack_rms else 1., 0.9 / peak if peak else 1.)
    return {'frames': len(pcm), 'sample_rate': rate, 'layer_count': len(layers), 'gain': gain,
            'attack_rms': attack_rms, 'peak': peak, 'playback_trim': trim}
