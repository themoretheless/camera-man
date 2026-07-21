#!/usr/bin/env python3
import pathlib
import struct
import sys
import zlib


def decode_png(path: pathlib.Path):
    raw = path.read_bytes()
    if raw[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path}: not a PNG")
    offset = 8
    width = height = color_type = None
    compressed = bytearray()
    while offset < len(raw):
        length = struct.unpack(">I", raw[offset : offset + 4])[0]
        kind = raw[offset + 4 : offset + 8]
        data = raw[offset + 8 : offset + 8 + length]
        offset += 12 + length
        if kind == b"IHDR":
            width, height, depth, color_type = struct.unpack(">IIBB", data[:10])
            if depth != 8 or color_type not in (2, 6):
                raise SystemExit(f"{path}: unsupported PNG encoding")
        elif kind == b"IDAT":
            compressed.extend(data)
        elif kind == b"IEND":
            break
    channels = 4 if color_type == 6 else 3
    scanlines = zlib.decompress(compressed)
    expected = height * (1 + width * channels)
    if len(scanlines) != expected:
        raise SystemExit(f"{path}: unexpected interlaced or truncated pixels")
    row_bytes = width * channels
    decoded = bytearray()
    previous = bytearray(row_bytes)
    offset = 0
    for _ in range(height):
        filter_type = scanlines[offset]
        offset += 1
        row = bytearray(scanlines[offset : offset + row_bytes])
        offset += row_bytes
        for index in range(row_bytes):
            left = row[index - channels] if index >= channels else 0
            above = previous[index]
            upper_left = previous[index - channels] if index >= channels else 0
            if filter_type == 0:
                predictor = 0
            elif filter_type == 1:
                predictor = left
            elif filter_type == 2:
                predictor = above
            elif filter_type == 3:
                predictor = (left + above) // 2
            elif filter_type == 4:
                predictor = paeth(left, above, upper_left)
            else:
                raise SystemExit(f"{path}: unsupported PNG filter {filter_type}")
            row[index] = (row[index] + predictor) & 0xFF
        decoded.extend(row)
        previous = row
    return width, height, channels, decoded


def paeth(left: int, above: int, upper_left: int) -> int:
    prediction = left + above - upper_left
    left_distance = abs(prediction - left)
    above_distance = abs(prediction - above)
    upper_left_distance = abs(prediction - upper_left)
    if left_distance <= above_distance and left_distance <= upper_left_distance:
        return left
    if above_distance <= upper_left_distance:
        return above
    return upper_left


def expected_fixtures():
    logical = {
        "workspace": (1120, 720),
        "setup": (920, 560),
        "empty": (920, 560),
        "mixed": (1120, 720),
        "disconnected": (1120, 720),
        "install-error": (920, 560),
        "running": (1440, 900),
    }
    expected = {}
    for state, dimensions in logical.items():
        for scale in (1, 2):
            expected[f"{state}-{scale}x.png"] = (
                dimensions[0] * scale,
                dimensions[1] * scale,
            )
    expected.update(
        {
            "pseudo-minimum.png": (920, 560),
            "pseudo-default.png": (1120, 720),
            "pseudo-large.png": (1440, 900),
        }
    )
    return expected


def verify_nonblank(path, width, height, channels, pixels):
    colors = set()
    quadrants = [set(), set(), set(), set()]
    minimum_alpha = 255
    x_step = max(1, width // 64)
    y_step = max(1, height // 64)
    for y in range(0, height, y_step):
        for x in range(0, width, x_step):
            pixel = (y * width + x) * channels
            color = tuple(pixels[pixel : pixel + 3])
            colors.add(color)
            quadrant = (2 if y >= height // 2 else 0) + (1 if x >= width // 2 else 0)
            quadrants[quadrant].add(color)
            if channels == 4:
                minimum_alpha = min(minimum_alpha, pixels[pixel + 3])
    if len(colors) < 8 or any(len(quadrant) < 3 for quadrant in quadrants):
        raise SystemExit(f"{path}: screenshot appears blank")
    if channels == 4 and minimum_alpha < 250:
        raise SystemExit(f"{path}: screenshot framebuffer is not opaque")


def main():
    directory = pathlib.Path(sys.argv[1])
    for name, expected_size in expected_fixtures().items():
        path = directory / name
        if not path.is_file():
            raise SystemExit(f"missing fixture {path}")
        width, height, channels, pixels = decode_png(path)
        if (width, height) != expected_size:
            raise SystemExit(
                f"{path}: expected {expected_size[0]}x{expected_size[1]}, got {width}x{height}"
            )
        verify_nonblank(path, width, height, channels, pixels)
        print(f"verified {path}: {width}x{height}")


if __name__ == "__main__":
    main()
